import "./preview.css";

export function OrbitalBeaconPreview() {
  return (
    <span class="story-theme-preview orbital-beacon-preview" aria-hidden="true">
      <span class="orbital-beacon-preview__orbit orbital-beacon-preview__orbit--outer" />
      <span class="orbital-beacon-preview__orbit orbital-beacon-preview__orbit--inner" />
      <span class="orbital-beacon-preview__sweep" />
      <span class="orbital-beacon-preview__core" />
      <span class="orbital-beacon-preview__blip" />
    </span>
  );
}
