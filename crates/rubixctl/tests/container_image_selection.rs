use rubixctl::container_image::select_container_image;
use rubixctl::contract::InstallOptions;

#[test]
fn test_select_container_image_default() {
    let mut options = InstallOptions::default();
    options.version = "v1.2.3".to_string();

    let image = select_container_image(&options, None).unwrap();
    assert_eq!(image, "portainer/kubesolo:v1.2.3");
}

#[test]
fn test_select_container_image_override() {
    let mut options = InstallOptions::default();
    options.version = "v1.2.3".to_string();
    options.container_image = Some("custom/image:latest".to_string());

    let image = select_container_image(&options, None).unwrap();
    assert_eq!(image, "custom/image:latest");
}
