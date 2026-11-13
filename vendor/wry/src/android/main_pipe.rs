// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{Error, InitializationScript, RGBA};
use crossbeam_channel::*;
use jni::{
  JNIEnv, JavaVM,
  errors::Result as JniResult,
  objects::{GlobalRef, JMap, JObject, JString},
};
use std::{
  collections::{BTreeMap, HashMap},
  ffi::c_void,
  os::unix::prelude::*,
  sync::{Arc, LazyLock, Mutex},
};

use super::{EVAL_CALLBACKS, EVAL_ID_GENERATOR, EvalCallback, WebviewId, find_class};

pub type ActivityId = i32;

static CHANNEL: LazyLock<(
  Sender<(ActivityId, WebViewMessage)>,
  Receiver<(ActivityId, WebViewMessage)>,
)> = LazyLock::new(|| bounded(8));
pub static MAIN_PIPE: LazyLock<[OwnedFd; 2]> = LazyLock::new(|| {
  let mut pipe: [RawFd; 2] = Default::default();
  unsafe { libc::pipe(pipe.as_mut_ptr()) };
  unsafe { pipe.map(|fd| OwnedFd::from_raw_fd(fd)) }
});

#[derive(Clone)]
pub struct ActivityProxy {
  pub activity: GlobalRef,
  pub window_manager: GlobalRef,
  /// The most recently created webview. Legacy single-slot access for
  /// activity-level paths.
  pub webview: Option<GlobalRef>,
  /// All live webviews on this activity, keyed by webview id. Tauri
  /// mobile puts multiple webview windows on one activity; routing
  /// per-webview operations through the single legacy slot delivered
  /// every eval (IPC responses, events) to the last-created webview.
  pub webviews: HashMap<WebviewId, GlobalRef>,
  pub java_vm: *mut c_void,
}

unsafe impl Send for ActivityProxy {}

impl ActivityProxy {
  pub fn new(vm: JavaVM, activity: GlobalRef, window_manager: GlobalRef) -> Self {
    Self {
      activity,
      window_manager,
      webview: None,
      webviews: HashMap::new(),
      java_vm: vm.get_java_vm_pointer() as *mut _,
    }
  }
}

static ACTIVITY_PROXY: LazyLock<Mutex<BTreeMap<ActivityId, ActivityProxy>>> =
  LazyLock::new(|| Mutex::new(BTreeMap::new()));

pub fn activity_proxy(id: ActivityId) -> Option<ActivityProxy> {
  ACTIVITY_PROXY.lock().unwrap().get(&id).cloned()
}

fn remove_activity_proxy(id: ActivityId) {
  ACTIVITY_PROXY.lock().unwrap().remove(&id);
}

pub fn register_activity_proxy(
  vm: JavaVM,
  id: ActivityId,
  activity: GlobalRef,
  window_manager: GlobalRef,
) {
  let mut activity_proxy = ACTIVITY_PROXY.lock().unwrap();
  if let Some(proxy) = activity_proxy.get_mut(&id) {
    proxy.activity = activity;
    proxy.window_manager = window_manager;
    proxy.java_vm = vm.get_java_vm_pointer() as *mut _;
  } else {
    let proxy = ActivityProxy::new(vm, activity, window_manager);
    activity_proxy.insert(id, proxy);
  }
}

pub fn activity_id_for_window_manager(window_manager: JObject) -> Option<ActivityId> {
  for (activity_id, proxy) in ACTIVITY_PROXY.lock().unwrap().iter() {
    let vm = unsafe { JavaVM::from_raw(proxy.java_vm.cast()) }.unwrap();
    let mut env = vm.attach_current_thread_as_daemon().unwrap();
    let equals = env
      .call_method(
        proxy.window_manager.as_obj(),
        "equals",
        "(Ljava/lang/Object;)Z",
        &[(&window_manager).into()],
      )
      .and_then(|v| v.z())
      .unwrap_or_default();
    if equals {
      return Some(*activity_id);
    }
  }
  None
}

pub fn first_activity_id() -> Option<ActivityId> {
  ACTIVITY_PROXY.lock().unwrap().keys().next().cloned()
}

pub fn get_webview(activity_id: ActivityId) -> Option<GlobalRef> {
  ACTIVITY_PROXY
    .lock()
    .unwrap()
    .get(&activity_id)?
    .webview
    .as_ref()
    .cloned()
}

/// Resolve a specific webview on an activity by its id.
pub fn get_webview_by_id(activity_id: ActivityId, webview_id: &WebviewId) -> Option<GlobalRef> {
  ACTIVITY_PROXY
    .lock()
    .unwrap()
    .get(&activity_id)?
    .webviews
    .get(webview_id)
    .cloned()
}

/// Drop a per-webview reference (webview destroyed) and repoint the
/// legacy single slot at any survivor.
pub fn remove_webview(activity_id: ActivityId, webview_id: &WebviewId) {
  if let Some(proxy) = ACTIVITY_PROXY.lock().unwrap().get_mut(&activity_id) {
    proxy.webviews.remove(webview_id);
    proxy.webview = proxy.webviews.values().next().cloned();
  }
}

pub struct MainPipe<'a> {
  pub env: JNIEnv<'a>,
}

impl<'a> MainPipe<'a> {
  pub(crate) fn send(activity_id: ActivityId, message: WebViewMessage) {
    const BYTE: [u8; 1] = [0];
    if CHANNEL.0.send((activity_id, message)).is_ok() {
      unsafe {
        libc::write(
          MAIN_PIPE[1].as_raw_fd(),
          BYTE.as_ptr() as *const _,
          BYTE.len(),
        )
      };
    }
  }

  pub fn recv(&mut self) -> JniResult<()> {
    let package = super::PACKAGE.get().unwrap();
    while let Ok((activity_id, message)) = CHANNEL.1.try_recv() {
      self.handle_message(package, activity_id, message)?;
    }
    Ok(())
  }

  fn handle_message(
    &mut self,
    package: &str,
    activity_id: ActivityId,
    message: WebViewMessage,
  ) -> JniResult<()> {
    match message {
      WebViewMessage::CreateWebView(attrs) => {
        let Some(ActivityProxy { activity, .. }) = activity_proxy(activity_id) else {
          #[cfg(debug_assertions)]
          eprintln!("no activity found for activity id: {}", activity_id);
          return Ok(());
        };
        let CreateWebViewAttributes {
          url,
          html,
          #[cfg(any(debug_assertions, feature = "devtools"))]
          devtools,
          transparent,
          background_color,
          headers,
          on_webview_created,
          autoplay,
          user_agent,
          initialization_scripts,
          id,
          javascript_disabled,
          ..
        } = attrs;
        // Keep the Rust-side id: `id` is shadowed by a JString below for
        // the constructor call, but the per-webview map needs the String.
        let webview_id: WebviewId = id.clone();

        let string_class = self.env.find_class("java/lang/String")?;
        let initialization_scripts_array = self.env.new_object_array(
          initialization_scripts.len() as i32,
          string_class,
          self.env.new_string("")?,
        )?;
        for (i, init_script) in initialization_scripts.into_iter().enumerate() {
          self.env.set_object_array_element(
            &initialization_scripts_array,
            i as i32,
            self.env.new_string(init_script.script)?,
          )?;
        }
        let id = self.env.new_string(id)?;
        // Create webview
        let rust_webview_class =
          find_class(&mut self.env, &activity, format!("{package}/RustWebView"))?;
        let webview = self.env.new_object(
          &rust_webview_class,
          "(Landroid/content/Context;[Ljava/lang/String;Ljava/lang/String;)V",
          &[
            (&activity).into(),
            (&initialization_scripts_array).into(),
            (&id).into(),
          ],
        )?;
        // get settings
        let web_settings = self
          .env
          .call_method(
            &webview,
            "getSettings",
            "()Landroid/webkit/WebSettings;",
            &[],
          )?
          .l()?;
        // set media autoplay
        self.env.call_method(
          &web_settings,
          "setMediaPlaybackRequiresUserGesture",
          "(Z)V",
          &[(!autoplay).into()],
        )?;
        // set user-agent
        if let Some(user_agent) = user_agent {
          let user_agent = self.env.new_string(user_agent)?;
          self.env.call_method(
            &web_settings,
            "setUserAgentString",
            "(Ljava/lang/String;)V",
            &[(&user_agent).into()],
          )?;
        }

        // disable javascript
        if javascript_disabled {
          self.env.call_method(
            &web_settings,
            "setJavaScriptEnabled",
            "(Z)V",
            &[false.into()],
          )?;
        }

        let webview_class_name = format!("{package}/RustWebView");
        self.env.call_method(
          &activity,
          "setWebView",
          format!("(L{webview_class_name};)V"),
          &[(&webview).into()],
        )?;
        // Navigation
        if let Some(u) = url {
          if let Ok(url) = self.env.new_string(u) {
            load_url(&mut self.env, &webview, &url, headers, true)?;
          }
        } else if let Some(h) = html {
          if let Ok(html) = self.env.new_string(h) {
            load_html(&mut self.env, &webview, &html)?;
          }
        }
        // Enable devtools
        #[cfg(any(debug_assertions, feature = "devtools"))]
        self.env.call_static_method(
          &rust_webview_class,
          "setWebContentsDebuggingEnabled",
          "(Z)V",
          &[devtools.into()],
        )?;
        if transparent {
          set_background_color(&mut self.env, &webview, (0, 0, 0, 0))?;
        } else if let Some(color) = background_color {
          set_background_color(&mut self.env, &webview, color)?;
        }
        // Create and set webview client
        let client_class_name = format!("{package}/RustWebViewClient");
        let rust_webview_client_class =
          find_class(&mut self.env, &activity, client_class_name.clone())?;
        let webview_client = self.env.new_object(
          &rust_webview_client_class,
          format!("(L{webview_class_name};Landroid/content/Context;)V"),
          &[(&webview).into(), (&activity).into()],
        )?;
        self.env.call_method(
          &webview,
          "setWebViewClient",
          "(Landroid/webkit/WebViewClient;)V",
          &[(&webview_client).into()],
        )?;
        // Create and set webchrome client
        let rust_webchrome_client_class = find_class(
          &mut self.env,
          &activity,
          format!("{package}/RustWebChromeClient"),
        )?;
        let web_chrome_client = self.env.new_object(
          &rust_webchrome_client_class,
          format!("(L{package}/WryActivity;Ljava/lang/String;)V"),
          &[(&activity).into(), (&id).into()],
        )?;
        self.env.call_method(
          &webview,
          "setWebChromeClient",
          "(Landroid/webkit/WebChromeClient;)V",
          &[(&web_chrome_client).into()],
        )?;

        // Ipc registers itself as `window.ipc`
        let ipc_class = find_class(&mut self.env, &activity, format!("{package}/Ipc"))?;
        self.env.new_object(
          ipc_class,
          format!("(L{webview_class_name};L{client_class_name};)V"),
          &[(&webview).into(), (&webview_client).into()],
        )?;

        // SURF PATCH: only the FIRST webview becomes the activity content
        // view; setContentView on every creation detaches the previous
        // webview, and a detached WebView silently drops every
        // evaluateJavascript (the Android multi-webview eval black hole).
        // Additional webviews stack into the android.R.id.content frame.
        {
          let was_first = {
            let proxies = ACTIVITY_PROXY.lock().unwrap();
            proxies.get(&activity_id).map(|p| p.webviews.is_empty()).unwrap_or(true)
          };
          if was_first {
            self.env.call_method(
              &activity,
              "setContentView",
              "(Landroid/view/View;)V",
              &[(&webview).into()],
            )?;
          } else {
            // android.R.id.content = 16908290; its ContentFrameLayout is a
            // ViewGroup, and addView(View) stacks the new webview on top.
            let content_frame = self
              .env
              .call_method(
                &activity,
                "findViewById",
                "(I)Landroid/view/View;",
                &[16908290.into()],
              )
              .and_then(|v| v.l())?;
            self.env.call_method(
              &content_frame,
              "addView",
              "(Landroid/view/View;)V",
              &[(&webview).into()],
            )?;
          }
        }

        if let Some(on_webview_created) = on_webview_created {
          if let Err(_e) = on_webview_created(super::Context {
            env: &mut self.env,
            activity: &activity,
            webview: &webview,
          }) {
            #[cfg(feature = "tracing")]
            tracing::warn!("failed to run webview created hook: {_e}");
          }
        }
        let webview: GlobalRef = self.env.new_global_ref(webview)?;

        let mut proxies = ACTIVITY_PROXY.lock().unwrap();
        let proxy = proxies.get_mut(&activity_id).unwrap();
        proxy.webview = Some(webview.clone());
        proxy.webviews.insert(webview_id, webview);
      }
      WebViewMessage::ForWebview { webview_id, op } => {
        let Some(webview) = get_webview_by_id(activity_id, &webview_id) else {
          #[cfg(debug_assertions)]
          eprintln!("no webview {webview_id} on activity {activity_id}; dropping op");
          return Ok(());
        };
        match op {
          WebviewOp::Eval(script, callback) => {
            let id = EVAL_ID_GENERATOR.next() as i32;

            #[cfg(feature = "tracing")]
            let span = std::sync::Mutex::new(Some(SendEnteredSpan(
              tracing::debug_span!("wry::eval").entered(),
            )));

            EVAL_CALLBACKS
              .get_or_init(Default::default)
              .lock()
              .unwrap()
              .insert(
                id,
                Box::new(move |result| {
                  #[cfg(feature = "tracing")]
                  span.lock().unwrap().take();

                  if let Some(callback) = &callback {
                    callback(result);
                  }
                }),
              );

            let s = self.env.new_string(script)?;
            self.env.call_method(
              webview.as_obj(),
              "evalScript",
              "(ILjava/lang/String;)V",
              &[id.into(), (&s).into()],
            )?;
          }
          WebviewOp::SetBackgroundColor(background_color) => {
            set_background_color(&mut self.env, webview.as_obj(), background_color)?;
          }
          WebviewOp::GetUrl(tx) => {
            let url = self
              .env
              .call_method(webview.as_obj(), "getUrl", "()Ljava/lang/String;", &[])
              .and_then(|v| v.l())
              .and_then(|s| {
                let s = JString::from(s);
                self
                  .env
                  .get_string(&s)
                  .map(|v| v.to_string_lossy().to_string())
              })
              .unwrap_or_default();

            tx.send(url).unwrap()
          }
          WebviewOp::LoadUrl(url, headers) => {
            let url = self.env.new_string(url)?;
            load_url(&mut self.env, webview.as_obj(), &url, headers, false)?;
          }
          WebviewOp::ClearAllBrowsingData => {
            self
              .env
              .call_method(&webview, "clearAllBrowsingData", "()V", &[])?;
          }
          WebviewOp::LoadHtml(html) => {
            let html = self.env.new_string(html)?;
            load_html(&mut self.env, webview.as_obj(), &html)?;
          }
          WebviewOp::Reload => {
            reload(&mut self.env, webview.as_obj())?;
          }
          WebviewOp::GoForward => {
            go_forward(&mut self.env, webview.as_obj())?;
          }
          WebviewOp::GoBack => {
            go_back(&mut self.env, webview.as_obj())?;
          }
          WebviewOp::CanGoForward(tx) => {
            tx.send(can_go_forward(&mut self.env, webview.as_obj())?)
              .unwrap();
          }
          WebviewOp::CanGoBack(tx) => {
            tx.send(can_go_back(&mut self.env, webview.as_obj())?)
              .unwrap();
          }
          WebviewOp::GetCookies(tx, url) => {
            let url = self.env.new_string(url)?;
            let cookies = self
              .env
              .call_method(
                &webview,
                "getCookies",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[(&url).into()],
              )
              .and_then(|v| v.l())
              .and_then(|s| {
                let s = JString::from(s);
                self
                  .env
                  .get_string(&s)
                  .map(|v| v.to_string_lossy().to_string())
              })
              .unwrap_or_default();

            tx.send(
              cookies
                .split("; ")
                .flat_map(|c| cookie::Cookie::parse(c.to_string()))
                .collect(),
            )
            .unwrap();
          }
        }
      }
      WebViewMessage::GetWebViewVersion(tx) => {
        if let Some(activity) = activity_proxy(activity_id).map(|p| p.activity) {
          match self
            .env
            .call_method(activity, "getVersion", "()Ljava/lang/String;", &[])
            .and_then(|v| v.l())
            .and_then(|s| {
              let s = JString::from(s);
              self
                .env
                .get_string(&s)
                .map(|v| v.to_string_lossy().to_string())
            }) {
            Ok(version) => {
              tx.send(Ok(version)).unwrap();
            }
            Err(e) => tx.send(Err(e.into())).unwrap(),
          }
        } else {
          tx.send(Err(Error::ActivityNotFound)).unwrap();
        }
      }
      WebViewMessage::Jni(f) => {
        match activity_proxy(activity_id).map(|p| (p.activity.clone(), p.webview)) {
          Some((activity, Some(webview))) => {
            f(&mut self.env, &activity, webview.as_obj());
          }
          Some((activity, None)) => {
            f(&mut self.env, &activity, &JObject::null());
          }
          _ => {
            f(&mut self.env, &JObject::null(), &JObject::null());
          }
        }
      }
      WebViewMessage::OnDestroy {
        activity_id,
        webview_id,
        is_changing_configurations,
      } => {
        // keep our webview references (callbacks etc) alive if the activity is going to be recreated due to configuration changes
        // e.g. rotation, multi-window mode change, etc
        if !is_changing_configurations {
          super::destroy_webview(activity_id, &webview_id);
          remove_activity_proxy(activity_id);
        }
      }
    }
    Ok(())
  }
}

fn load_url<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  url: &JString<'a>,
  headers: Option<http::HeaderMap>,
  main_thread: bool,
) -> JniResult<()> {
  let function = if main_thread {
    "loadUrlMainThread"
  } else {
    "loadUrl"
  };
  if let Some(headers) = headers {
    let obj = env.new_object("java/util/HashMap", "()V", &[])?;
    let headers_map = {
      let headers_map = JMap::from_env(env, &obj)?;
      for (name, value) in headers.iter() {
        let key = env.new_string(name)?;
        let value = env.new_string(value.to_str().unwrap_or_default())?;
        headers_map.put(env, &key, &value)?;
      }
      headers_map
    };
    env.call_method(
      webview,
      function,
      "(Ljava/lang/String;Ljava/util/Map;)V",
      &[url.into(), (&headers_map).into()],
    )?;
  } else {
    env.call_method(webview, function, "(Ljava/lang/String;)V", &[url.into()])?;
  }
  Ok(())
}

fn load_html<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>, html: &JString<'a>) -> JniResult<()> {
  env.call_method(
    webview,
    "loadHTMLMainThread",
    "(Ljava/lang/String;)V",
    &[html.into()],
  )?;
  Ok(())
}

fn reload<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>) -> JniResult<()> {
  env.call_method(webview, "reload", "()V", &[])?;
  Ok(())
}

fn go_forward<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>) -> JniResult<()> {
  env.call_method(webview, "goForward", "()V", &[])?;
  Ok(())
}

fn go_back<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>) -> JniResult<()> {
  env.call_method(webview, "goBack", "()V", &[])?;
  Ok(())
}

fn can_go_forward<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>) -> JniResult<bool> {
  Ok(env.call_method(webview, "canGoForward", "()Z", &[])?.z()?)
}

fn can_go_back<'a>(env: &mut JNIEnv<'a>, webview: &JObject<'a>) -> JniResult<bool> {
  Ok(env.call_method(webview, "canGoBack", "()Z", &[])?.z()?)
}

fn set_background_color<'a>(
  env: &mut JNIEnv<'a>,
  webview: &JObject<'a>,
  (r, g, b, a): RGBA,
) -> JniResult<()> {
  let color = (a as i32) << 24 | (r as i32) << 16 | (g as i32) << 8 | (b as i32);
  env.call_method(webview, "setBackgroundColor", "(I)V", &[color.into()])?;
  Ok(())
}

pub(crate) enum WebViewMessage {
  CreateWebView(CreateWebViewAttributes),
  /// An operation that targets one specific webview on the activity.
  ForWebview { webview_id: WebviewId, op: WebviewOp },
  GetWebViewVersion(Sender<Result<String, Error>>),
  Jni(Box<dyn FnOnce(&mut JNIEnv, &JObject, &JObject) + Send>),
  OnDestroy {
    activity_id: ActivityId,
    webview_id: WebviewId,
    is_changing_configurations: bool,
  },
}

/// Per-webview operations; routed through `ForWebview` so they reach the
/// exact webview (see `ActivityProxy::webviews`).
pub(crate) enum WebviewOp {
  Eval(String, Option<EvalCallback>),
  SetBackgroundColor(RGBA),
  GetUrl(Sender<String>),
  GetCookies(Sender<Vec<cookie::Cookie<'static>>>, String),
  LoadUrl(String, Option<http::HeaderMap>),
  LoadHtml(String),
  Reload,
  GoForward,
  GoBack,
  CanGoForward(Sender<bool>),
  CanGoBack(Sender<bool>),
  ClearAllBrowsingData,
}

#[derive(Clone)]
pub(crate) struct CreateWebViewAttributes {
  pub id: String,
  pub url: Option<String>,
  pub html: Option<String>,
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub devtools: bool,
  pub transparent: bool,
  pub background_color: Option<RGBA>,
  pub headers: Option<http::HeaderMap>,
  pub autoplay: bool,
  pub on_webview_created:
    Option<Arc<dyn Fn(super::Context) -> JniResult<()> + Send + Sync + 'static>>,
  pub user_agent: Option<String>,
  pub initialization_scripts: Vec<InitializationScript>,
  pub javascript_disabled: bool,
}

// SAFETY: only use this when you are sure the span will be dropped on the same thread it was entered
#[cfg(feature = "tracing")]
struct SendEnteredSpan(tracing::span::EnteredSpan);

#[cfg(feature = "tracing")]
unsafe impl Send for SendEnteredSpan {}
