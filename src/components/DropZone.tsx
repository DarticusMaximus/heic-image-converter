import { IconDrop, IconFiles, IconFolder, IconShield } from "./Icons";

interface Props {
  onPickFiles: () => void;
  onPickFolder: () => void;
  busy: boolean;
}

export function DropZone({ onPickFiles, onPickFolder, busy }: Props) {
  return (
    <div className="dropzone">
      <div className="dropzone-glow" />
      <div className="dropzone-icon">
        <IconDrop size={30} />
      </div>
      <h1>Drop your images here</h1>
      <p className="dropzone-sub">
        HEIC, HEIF, JPEG, PNG, WebP, TIFF, BMP and GIF. Add single files or a whole
        folder &mdash; subfolders included.
      </p>
      <div className="dropzone-actions">
        <button className="btn primary" onClick={onPickFiles} disabled={busy}>
          <IconFiles size={18} />
          Choose images
        </button>
        <button className="btn" onClick={onPickFolder} disabled={busy}>
          <IconFolder size={18} />
          Choose folder
        </button>
      </div>
      <p className="privacy">
        <IconShield size={15} />
        Nothing is uploaded. Every conversion runs on this machine.
      </p>
    </div>
  );
}
