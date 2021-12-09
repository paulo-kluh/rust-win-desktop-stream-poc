#![windows_subsystem = "windows"]
use std::{
  ffi::c_void,
  ptr::{self, NonNull},
};

use windows as Windows;
use Windows::{
  core::{implement, Interface, IntoParam},
  Win32::{
    Foundation::HINSTANCE,
    Graphics::{
      Direct3D::D3D_DRIVER_TYPE_UNKNOWN,
      Direct3D11::{
        D3D11_CREATE_DEVICE_DEBUG, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
      },
    },
    Graphics::{
      Direct3D11::{D3D11CreateDevice, ID3D11Multithread},
      Dxgi::{CreateDXGIFactory1, IDXGIFactory1},
    },
    Media::MediaFoundation::{
      IMFActivate, IMFAsyncCallback, IMFAsyncResult, IMFMediaEventGenerator, IMFTransform,
      MFCreateDXGIDeviceManager, MFMediaType_Video, MFStartup, MFTEnum2, MFVideoFormat_H264,
      MFVideoFormat_NV12, MFASYNC_BLOCKING_CALLBACK, MFASYNC_CALLBACK_QUEUE_MULTITHREADED,
      MFSTARTUP_FULL, MFT_CATEGORY_VIDEO_ENCODER, MFT_ENUM_FLAG_ASYNCMFT, MFT_ENUM_FLAG_HARDWARE,
      MFT_ENUM_FLAG_SORTANDFILTER, MFT_MESSAGE_SET_D3D_MANAGER, MFT_REGISTER_TYPE_INFO,
      MF_API_VERSION, MF_SDK_VERSION, MF_TRANSFORM_ASYNC_UNLOCK,
    },
    System::Com::{CoInitializeEx, COINIT_MULTITHREADED},
  },
};

#[implement(Windows::Win32::Media::MediaFoundation::IMFAsyncCallback)]
struct WrappedIMFAsyncCallback();

#[allow(non_snake_case)]
impl WrappedIMFAsyncCallback {
  pub unsafe fn GetParameters(
    &self,
    pdwflags: *mut u32,
    pdwqueue: *mut u32,
  ) -> Windows::core::Result<()> {
    println!("GetParameters called");
    *pdwflags = MFASYNC_BLOCKING_CALLBACK;
    *pdwqueue = MFASYNC_CALLBACK_QUEUE_MULTITHREADED;
    Ok(())
    // Err(windows::core::Error::fast_error(windows::Win32::Foundation::E_NOTIMPL))
  }
  pub unsafe fn Invoke<'a, Param0: IntoParam<'a, IMFAsyncResult>>(
    &self,
    pasyncresult: Param0,
  ) -> Windows::core::Result<()> {
    println!("Event Received");
    Ok(())
  }
}

fn main() {
  unsafe { win_capture2() }
}

unsafe fn win_capture2() {
  CoInitializeEx(ptr::null(), COINIT_MULTITHREADED).expect("CoInitializeEx failed");
  let mf_sdk_version = (MF_SDK_VERSION << 16) | MF_API_VERSION;
  MFStartup(mf_sdk_version, MFSTARTUP_FULL).expect("MFStartup failed");
  let factory: IDXGIFactory1 = CreateDXGIFactory1().expect("CreateDXGIFactory1 failed");
  let adapter = factory.EnumAdapters1(0).expect("EnumAdapters1 failed");
  let (mut device_option, mut device_context_option) = (None, None);
  let (device, device_context) = D3D11CreateDevice(
    &adapter,
    D3D_DRIVER_TYPE_UNKNOWN,
    HINSTANCE::default(),
    D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_DEBUG,
    ptr::null_mut(),
    0,
    D3D11_SDK_VERSION,
    &mut device_option,
    ptr::null_mut(),
    &mut device_context_option,
  )
  .and_then(move |_| {
    Ok((
      device_option.expect("d3d11_device is None"),
      device_context_option.expect("device_context is None"),
    ))
  })
  .expect("D3D11CreateDevice failed");
  device
    .cast::<ID3D11Multithread>()
    .expect("Cast ID3D11Device to ID3D11Multithread failed")
    .SetMultithreadProtected(true);
  let (mut device_manager_reset_token, mut device_manager_option) = (0, None);
  let device_manager =
    MFCreateDXGIDeviceManager(&mut device_manager_reset_token, &mut device_manager_option)
      .and_then(|_| Ok(device_manager_option.expect("device_manager is None")))
      .expect("MFCreateDXGIDeviceManager failed");
  device_manager
    .ResetDevice(device, device_manager_reset_token)
    .expect("ResetDevice failed");
  let device_handler = device_manager
    .OpenDeviceHandle()
    .expect("OpenDeviceHandle failed");
  device_manager
    .TestDevice(device_handler)
    .expect("TestDevice failed");
  let input_type = MFT_REGISTER_TYPE_INFO {
    guidMajorType: MFMediaType_Video,
    guidSubtype: MFVideoFormat_NV12,
  };
  let output_type = MFT_REGISTER_TYPE_INFO {
    guidMajorType: MFMediaType_Video,
    guidSubtype: MFVideoFormat_H264,
  };
  let mut array: [Option<IMFActivate>; 10] = Default::default();
  let mut mf_activate_ptr = array.as_mut_ptr();
  let mut mf_activate_size = 0u32;
  let activate = MFTEnum2(
    MFT_CATEGORY_VIDEO_ENCODER,
    MFT_ENUM_FLAG_ASYNCMFT.0 as u32
      | MFT_ENUM_FLAG_HARDWARE.0 as u32
      | MFT_ENUM_FLAG_SORTANDFILTER.0 as u32,
    &input_type,
    &output_type,
    None,
    &mut mf_activate_ptr,
    &mut mf_activate_size,
  )
  .and_then(|_| {
    Ok(
      std::slice::from_raw_parts(mf_activate_ptr, mf_activate_size as usize)[0]
        .as_ref()
        .expect("MFActivate at index 0"),
    )
  })
  .expect("MFTEnum2 failed");
  let mf_transform: IMFTransform = activate.ActivateObject().expect("ActivateObject failed");
  let mf_attributes = mf_transform.GetAttributes().expect("GetAttributes failed");
  mf_attributes
    .SetUINT32(&MF_TRANSFORM_ASYNC_UNLOCK, true as u32)
    .expect("SetUINT32 MF_TRANSFORM_ASYNC_UNLOCK failed");
  let event_generator = mf_transform
    .cast::<IMFMediaEventGenerator>()
    .expect("cast IMFMediaEventGenerator failed");
  let imf_cb: IMFAsyncCallback = WrappedIMFAsyncCallback().into();
  event_generator
    .BeginGetEvent(imf_cb, None)
    .expect("BeginGetEvent failed");
  let device_manager_ptr =
    &mut std::mem::transmute(device_manager) as *mut NonNull<c_void> as usize;
  mf_transform
    .ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, device_manager_ptr)
    .expect("ProcessMessage MFT_MESSAGE_SET_D3D_MANAGER failed");
}
