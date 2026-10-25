export interface Profile {
  id: string;
  /** Display name — may be a custom name chosen in this app. */
  name: string;
  /**
   * The profile's `Name` exactly as stored in `profiles.ini`.
   * Zen resolves launches by this value, so it is what must be passed to
   * `launch_profile`, not `name`.
   */
  zen_name: string;
  path: string;
  is_default: boolean;
  avatar?: string;
}