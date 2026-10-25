import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Plus, RotateCcw, TriangleAlert } from "lucide-react";

import ProfileCard from "./ProfileCard";
import NewProfileModal from "./NewProfileModal";
import EmptyState from "./EmptyState";
import type { Profile } from "../types/profile";

interface ProfileGridProps {
  showNewProfileModal: boolean;
  setShowNewProfileModal: (show: boolean) => void;
}

export default function ProfileGrid({
  showNewProfileModal,
  setShowNewProfileModal,
}: ProfileGridProps) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    loadProfiles();
  }, []);

  async function loadProfiles() {
    setLoadError(null);

    try {
      const raw = await invoke<Profile[]>("get_profiles");

      // For each profile that has an avatar filename, convert it to a
      // base64 data URL via load_avatar. ProfileCard receives a ready-to-use
      // <img src> value — no filesystem paths ever reach React.
      const withAvatars = await Promise.all(
        raw.map(async (p) => {
          if (!p.avatar) return p;
          try {
            const dataUrl = await invoke<string>("load_avatar", {
              filename: p.avatar,
            });
            return { ...p, avatar: dataUrl };
          } catch {
            // Avatar file missing or unreadable — show initials fallback.
            return { ...p, avatar: undefined };
          }
        })
      );

      setProfiles(withAvatars);
    } catch (error) {
      console.error("Failed to load profiles:", error);
      // Rust rejects with a plain string — show it instead of the empty
      // state, which would wrongly invite the user to create a profile.
      setLoadError(typeof error === "string" ? error : String(error));
    } finally {
      setLoading(false);
    }
  }

  /**
   * Re-runs loadProfiles after a failure. Resetting `loading` first is
   * necessary because the initial attempt already flipped it to false.
   */
  async function handleRetry() {
    setLoading(true);
    await loadProfiles();
  }

  async function handleProfileClick(profile: Profile) {
    await invoke("launch_profile", {
      // Launch by the profile's real profiles.ini name. `profile.name` is the
      // display name and diverges from it as soon as the profile is renamed,
      // which would ask Zen to open a profile that does not exist.
      profileId: profile.zen_name,
    });
  }

  async function handleAvatarClick(profile: Profile) {
    try {
      // Step 1: open native picker, copy file, get filename back
      const filename = await invoke<string | null>("select_avatar", {
        profileId: profile.id,
      });

      if (!filename) return; // user cancelled

      // Step 2: convert filename → data URL (all FS access in Rust)
      const dataUrl = await invoke<string>("load_avatar", { filename });

      // Step 3: update only this profile in state — no full reload
      setProfiles((prev) =>
        prev.map((p) =>
          p.id === profile.id ? { ...p, avatar: dataUrl } : p
        )
      );
    } catch (error) {
      console.error("Failed to set avatar:", error);
    }
  }

  async function handleAvatarRemove(profile: Profile) {
    try {
      await invoke("remove_avatar", { profileId: profile.id });
      setProfiles((prev) =>
        prev.map((p) =>
          p.id === profile.id ? { ...p, avatar: undefined } : p
        )
      );
    } catch (error) {
      console.error("Failed to remove avatar:", error);
    }
  }

  async function handleRename(profile: Profile, newName: string) {
    try {
      await invoke("rename_profile", {
        profileId: profile.id,
        displayName: newName,
      });

      // Update state locally
      setProfiles((prev) =>
        prev.map((p) =>
          p.id === profile.id ? { ...p, name: newName } : p
        )
      );
    } catch (error) {
      console.error("Failed to rename profile:", error);
    }
  }

  /**
   * Called by NewProfileModal when the user confirms.
   * Throws a string on failure so the modal can display it inline.
   */
  async function handleCreate(name: string, launchAfter: boolean) {
    // invoke throws the Rust Err(String) as a string — modal catches it.
    const newProfile = await invoke<Profile>("create_profile", { name });

    // Append to state; no avatar conversion needed (fresh profile has none).
    setProfiles((prev) => [...prev, newProfile]);
    setShowNewProfileModal(false);

    if (launchAfter) {
      // Use zen_name rather than name so this stays correct even once the
      // profile carries a custom display name of its own.
      await invoke("launch_profile", { profileId: newProfile.zen_name });
    }
  }

  if (loading) {
    return (
      <div className="flex justify-center py-24 text-muted-foreground">
        Loading profiles...
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="flex flex-col items-center justify-center py-16 px-4 text-center animate-in fade-in duration-150 ease-out">
        <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-2xl border border-dashed border-border bg-card/30 text-muted-foreground/60">
          <TriangleAlert size={28} strokeWidth={1.5} />
        </div>

        <h3 className="text-lg font-medium tracking-tight text-foreground mb-2">
          Couldn&rsquo;t Load Profiles
        </h3>

        <p className="text-sm text-muted-foreground font-normal max-w-md mb-6 leading-relaxed break-words">
          {loadError}
        </p>

        <button
          onClick={handleRetry}
          className="
            flex items-center gap-1.5
            rounded-xl
            border border-border
            px-5 py-2
            text-sm font-medium text-foreground
            transition-colors duration-150
            hover:bg-card
            shadow-sm
          "
        >
          <RotateCcw size={14} />
          Try Again
        </button>
      </div>
    );
  }

  if (profiles.length === 0) {
    return (
      <>
        <EmptyState onCreateClick={() => setShowNewProfileModal(true)} />
        {showNewProfileModal && (
          <NewProfileModal
            onClose={() => setShowNewProfileModal(false)}
            onCreate={handleCreate}
          />
        )}
      </>
    );
  }

  return (
    <>
      <section className="mx-auto grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-6 max-w-fit px-6 pb-12 justify-items-center">
        {profiles.map((profile) => (
          <ProfileCard
            key={profile.id}
            profile={profile}
            onClick={handleProfileClick}
            onAvatarClick={handleAvatarClick}
            onAvatarRemove={handleAvatarRemove}
            onRename={handleRename}
          />
        ))}

        {/* ── New Profile Card ─────────────────────────────────────────────── */}
        <button
          id="new-profile-card"
          onClick={() => setShowNewProfileModal(true)}
          className="
            group
            w-[160px]
            h-[160px]
            cursor-pointer
            flex flex-col items-center justify-center
            rounded-3xl
            border border-dashed border-border
            bg-card/40
            p-5
            transition-all duration-200
            ease-out
            shadow-[0_4px_15px_-4px_rgba(0,0,0,0.05)]
            dark:shadow-none
            hover:border-primary/40
            hover:bg-card/85
            hover:shadow-[0_8px_25px_-6px_rgba(247,111,83,0.1)]
            dark:hover:shadow-[0_8px_30px_-8px_rgba(247,111,83,0.2)]
          "
        >
          <div
            className="
              mb-4 flex h-14 w-14 items-center justify-center
              rounded-full border border-dashed border-border
              text-muted-foreground
              transition-colors duration-200
              group-hover:border-foreground/40
              group-hover:text-foreground
            "
          >
            <Plus size={22} />
          </div>
          <span
            className="
              text-sm font-medium text-muted-foreground
              transition-colors duration-200
              group-hover:text-foreground
            "
          >
            New Profile
          </span>
        </button>
      </section>

      {/* ── Modal ──────────────────────────────────────────────────────────── */}
      {showNewProfileModal && (
        <NewProfileModal
          onClose={() => setShowNewProfileModal(false)}
          onCreate={handleCreate}
        />
      )}
    </>
  );
}