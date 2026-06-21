import { useUiStore } from "../stores/uiStore";
import { ColorPickerCard } from "./sidebar/ColorPickerCard";
import { PlaybackCard } from "./sidebar/PlaybackCard";
import { ProfilesCard } from "./sidebar/ProfilesCard";
import { SafetyCard } from "./sidebar/SafetyCard";
import { SimpleConfig } from "./sidebar/SimpleConfig";
import { TargetWindowCard } from "./sidebar/TargetWindowCard";

export function Sidebar() {
  const mode = useUiStore((s) => s.mode);

  return (
    <aside className="sidebar">
      {mode === "simple" ? (
        <SimpleConfig />
      ) : (
        <>
          <ProfilesCard />
          <PlaybackCard />
          <TargetWindowCard />
          <ColorPickerCard />
        </>
      )}
      <SafetyCard />
    </aside>
  );
}
