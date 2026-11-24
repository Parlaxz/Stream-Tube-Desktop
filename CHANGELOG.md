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
- **Read ahead** card: choose the forward and back video buffer sizes in MiB. Each open video tab keeps its own buffer in RAM.
- **App icon** card: pick the window, taskbar and in-app icon from the 21 ReVanced branding icons (AFN, MMT, Revancify, Vanced, Xisr and more), or keep **Default**. Icons show in a grid; the selected one is outlined and applied with **Save**.
- The chosen icon is used for the window and taskbar, desktop/Start/taskbar shortcuts, the Linux menu icon, and the logo inside the app.
- The top bar now shows only the chosen app icon (still opens Home) instead of the "Stream Tube" wordmark.

### Tabs & Navigation

- **Middle-click** a video or short — on home, subscriptions, search results, or the related/queue list — to open it in a new background tab, same as **Ctrl/Cmd+click**.
- **Ctrl/Cmd+click** or **middle-click** any sidebar item (Home, Shorts, Subscriptions, History, Playlists, Watch later, Downloads, channels, Explore, Settings) to open it in a new tab. New sidebar items get this automatically.
- Right-click a video thumbnail opens **Save to playlist**; middle-click now opens the video in a new tab instead of opening that dialog.
- Each tab now has its own player. Switching away from a video tab pauses it but keeps its engine, demuxer cache and exact position in RAM; switching back resumes it without downloading the loaded part again. Closing a tab frees its buffer.
- Opening a video in a background tab loads nothing until the tab is clicked. The tab shows the card title and channel icon only.
- Tabs persist across a full restart: routes, titles, channel icons and the active tab are restored. Video buffers cannot survive a process restart.
- The tab bar has a grid button, and **left+right click anywhere** opens the same tab overview. The grid fills 90% of the window in both directions and is scrollable.
- The grid shows video thumbnails (icons for other pages), with the channel avatar under each video thumbnail. Click a tile to switch to that tab.
- Hovering a video tab in the tab bar shows its thumbnail.
- The grid uses **6 columns** by default. The **Dynamic** toggle switches to a square-ish layout (`rows = floor(sqrt(n))`, `columns = ceil(n / rows)`). The choice is remembered.
- The 12-tab limit is removed.


### Home Page preservation

- The top-bar **pin** keeps the home feed through reloads: while it is on, a refresh keeps the page and scrolling only appends new videos.
- **Settings → Custom Features → Home Page preservation** chooses the mode:
  - **Pin button** — the top-bar pin decides (default).
  - **On hard refresh and close** — the feed is kept, but a manual refresh or an app restart reloads it.
  - **Off** — reload with the page.

### Subscription profiles

- Profiles group subscribed channels into separate feeds. Create them under **Subscriptions → Manage**; nothing is uploaded.
- Profiles appear in the **Subscriptions** feed and in the sidebar. Clicking a sidebar profile opens its feed and refreshes it.
- The sidebar **Subscriptions** section collapses with a chevron, and the collapsed state is kept between launches.
- Pinned playlists now sit above **Subscriptions** in the sidebar.
- **Settings → Custom Features → Subscription profiles** adds a **Refresh time** choice (default: 1 hour). A profile's feed reloads when its cache is older than that.
- **Import profiles** from a PocketTube subscriptions CSV (`Title, Link, Subscribers, Groups`). Grouped channels are subscribed and added to their profiles.

### Player & Scrolling

- **Scroll wheel over the video** changes the volume by 3% per notch. It works while the video is paused and in fullscreen. The up/down arrow keys still change the volume by 5%.
- **Middle-click empty space** on a page starts browser-style autoscroll. A middle click on a video still opens it in a new tab instead. Move the pointer up or down to scroll. The speed follows the distance from the click and has no upper limit. A marker shows where the click was. Another middle click, any other click, or leaving the window stops it.

### Thumbnails

- The `⋮` menu on video cards and the watch page now has **Copy thumbnail**, below **Copy link**. It copies the thumbnail image itself to the clipboard.
- Copied images also use the standard Windows `CF_DIB` format, so they paste into Explorer, Paint, .NET apps and clipboard history.

### Playlists

- Pin a YouTube playlist with the pin button on its card in **Playlists**; pinned playlists show in the sidebar.
- The sidebar lists only pinned playlists. A **Show N more** row reveals the unpinned ones.
- Hovering a video thumbnail shows **Watch later** and **Add to queue** buttons.
- **Save to playlist** lists local and YouTube playlists with checkboxes. The checkboxes show which playlists already contain the video, and those playlists are listed first.
- Left-click a playlist to save and close the dialog. Right-click its checkbox to toggle it without closing.
