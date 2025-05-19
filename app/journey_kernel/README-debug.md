# Debug Panel for MemoLanes Map

## Overview
The Debug Panel provides developer controls for testing different caching and rendering modes on the MemoLanes map.

## How to Use

### Enabling the Debug Panel
To enable the debug panel, simply add `debug=true` to your URL hash parameters:

```
http://yourdomain.com/path#debug=true
```

Or add it to existing parameters:

```
http://yourdomain.com/path#journey_id=123&debug=true
```

### Available Controls

#### Caching Mode
Controls how data is cached:
- **Auto**: System decides the best caching strategy
- **Performance**: Maximum caching for better performance
- **Light**: Minimum caching for lower memory usage

URL parameter: `cache=auto|performance|light`

#### Rendering Mode
Controls how the map layers are rendered:
- **Auto**: System decides the best rendering method
- **Canvas**: Uses Canvas API for rendering

URL parameter: `render=auto|canvas`

### Applying Settings
After selecting your desired options, click "Apply Settings" to update the URL with your chosen parameters. The page will not reload, but the map will update according to your settings.

### Closing the Panel
Click the "×" button in the top-right corner to close the panel and set `debug=false` in the URL.

## Implementation Notes

The debug panel is implemented in:
- `debug-panel.js` - Main panel functionality
- `debug-panel.css` - Panel styling

It's initialized in `index.js` when the map is loaded. 