# Changelog

## 0.9.0

### Custom Features

- New **Settings → Custom Features** section, between *Player* and *Premium features*.
- Configurable playback speed shortcuts, all respected at runtime:
  - **Increase speed** — default key `D`, with its own step amount (default `0.25`).
  - **Decrease speed** — default key `S`, with its own step amount (default `0.25`).
  - **Reset speed** — default key `R`, returns to `1.00x`.
  - **Default speed shortcut** — default key `G`, applies the configured default speed.
- **Default speed** box: sets the speed videos start at and what the `G` shortcut applies.
- Speed control in the playback bar: current speed between `−` and `+` buttons, placed left of the subtitles button behind a vertical divider. The `−`/`+` buttons use the configured step amounts, and clicking the speed opens the speed menu.
- Each shortcut press shows the same speed popup as before.

### Tabs & Navigation

- **Middle-click** a video or short — on home, subscriptions, search results, or the related/queue list — to open it in a new background tab, same as **Ctrl/Cmd+click**.
- **Ctrl/Cmd+click** or **middle-click** any sidebar item (Home, Shorts, Subscriptions, History, Playlists, Watch later, Downloads, channels, Explore, Settings) to open it in a new tab. New sidebar items get this automatically.
- Right-click a video thumbnail opens **Save to playlist**; middle-click now opens the video in a new tab instead of opening that dialog.

### Subscription profiles

- Profiles group subscribed channels into separate feeds. Create them under **Subscriptions → Manage**; nothing is uploaded.
- Profiles appear in the **Subscriptions** feed and in the sidebar. Clicking a sidebar profile opens its feed and refreshes it.
- The sidebar **Subscriptions** section collapses with a chevron, and the collapsed state is kept between launches.
- Pinned playlists now sit above **Subscriptions** in the sidebar.
- **Settings → Custom Features → Subscription profiles** adds a **Refresh time** choice (default: 1 hour). A profile's feed reloads when its cache is older than that.
- **Import profiles** from a PocketTube subscriptions CSV (`Title, Link, Subscribers, Groups`). Grouped channels are subscribed and added to their profiles.

### Player & Scrolling

- **Scroll wheel over the video** changes the volume by 3% per notch. It works while the video is paused and in fullscreen. The up/down arrow keys still change the volume by 5%.
- **Middle-click empty space** on a page starts browser-style autoscroll. Move the pointer up or down to scroll. The speed follows the distance from the click and has no upper limit. A marker shows where the click was. Another middle click, any other click, or leaving the window stops it.

### Thumbnails

- The `⋮` menu on video cards and the watch page now has **Copy thumbnail**, below **Copy link**. It copies the thumbnail image itself to the clipboard.
- Copied images also use the standard Windows `CF_DIB` format, so they paste into Explorer, Paint, .NET apps and clipboard history.


