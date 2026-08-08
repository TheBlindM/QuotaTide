import "./preview.css";

export function RisingWaterPreview() {
  return (
    <span class="story-theme-preview rising-water-preview" aria-hidden="true">
      <span class="rising-water-preview__valve" />
      <span class="rising-water-preview__water" />
      <span class="rising-water-preview__robot">
        <i />
      </span>
      <span class="rising-water-preview__tick rising-water-preview__tick--one" />
      <span class="rising-water-preview__tick rising-water-preview__tick--two" />
    </span>
  );
}
