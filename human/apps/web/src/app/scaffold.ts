export function human_web_app_scaffold() {
  return Object.freeze({
    application: "LayerX Human Interface",
    planes: ["/app", "/explorer"] as const,
  });
}
