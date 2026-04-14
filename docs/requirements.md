Vehicle Track/Telemetry Visualization Application

1. Environment Requirements
    1.1 Application shall be implemented in Rust
    1.2 Application shall be a stand-alone package
        1.2.1 Application shall not access internet assets after build
        1.2.2 All assets shall be contained within the executable or release directory
        1.2.3 Application shall not require installation on target system
    
2. Data Support Requirements
    2.1 Application shall accept generic data formats
        2.1.1 Application shall accept CSV files
        2.1.2 Application shall accept parquet files
        2.1.3 Application shall accept UDP streams
        2.1.4 Application shall accept ADSB data
        2.1.5 Application shall accept multiple files
    2.2 Application shall accept additional user-loaded map boundaries
        2.2.1 Common geographic map vector formats
        2.2.2 Provide Help Menu Documentation on how to format new boundary files
    
3. Interactivity
    3.1 Application shall have a top menu bar
    3.2 Application shall have a left-side pane
    3.3 Application shall have a main plotting area
    3.4 Application shall provide the user the ability to:
        3.4.1 Filter displayed data
            3.4.1.1 Conditionally, by any data attribute
            3.4.1.2 Geographically, by Geographic Boundary
            3.4.1.3 Temporally
            3.4.1.4 By selection of displayed data/properties
            3.4.1.5 By point selection and radial distance
        3.4.2 Conditionally set data color
        3.4.3 Conditionally set data transparency
        3.4.4 Conditionally set data point size
        3.4.5 Conditionally set 'hover' text information
        3.4.6 Aggregate data points
        3.4.7 Zoom and Pan on plots
        3.4.8 Set plot limits, labels, and scales
        3.4.9 Link multiple plots together
        3.4.10 Right Click on data for additional context menu
        3.4.11 Select data points within plot
            3.4.11.1 Single points
            3.4.11.2 Multiple Points (Ctrl-click)
            3.4.11.3 Multiple Points (Area Drag)
            3.4.11.4 Multiple Points (Geographic Boundary Select)
    3.5 Application shall treat static data sources as full data sets or as streaming data sets
        3.5.1 Application shall provide the user the ability to adjust playback speed
        3.5.2 Application shall provide the user the ability to adjust data timeout duration

4. Visualizations and Plot types
    4.1 Application shall support multiple plot types, including:
        4.1.1 Geographic Plotting: Lat/Long on Map Backgrounds
            4.1.1.1 Online access to open source map database
            4.1.1.2 Ability to zoom down to state-level geographic detail while offline
            4.1.1.3 Provide user ability to select between multiple map schemes, including:
                4.1.1.3.1 White Background, dark lines
                4.1.1.3.2 Black Background, light lines
                4.1.1.3.3 Black Background, greenscale lines
                4.1.1.3.4 Dark Blue Background, light lines
        4.1.2 Geographic Plotting: Map Boundaries
            4.1.2.1 Ability to aggregate/color data based on geographic boundaries
        4.1.3 Scatter Plots
        4.1.4 Bar Graph Plots
        4.1.5 Scroll Charts
            4.1.5.1 Continuously display streaming information
            4.1.5.2 Provide ability to add tripwires/thresholds to control color of chart

5. Session Persistence Requirements
    5.1 Application shall provide capability to save session
        5.1.1 Povide the user the option to preserve:
            5.1.1.1 Loaded Dataset (whether streamed or from flat file)
                5.1.1.1.1 A pointer to the reference data file
                5.1.1.1.2 A compressed extract of the data
            5.1.1.2 Plots and layout, including scales/zoom/legends/etc
            5.1.1.3 Filters
        5.1.2 Output should be a human-readable package (.tay file) with:
            5.1.2.1 Layout, plot, filter, display information
            5.1.2.2 Data, either file reference or compressed binary/payload

6. Top Menu
    6.1 File Menu 
    6.2 Data Sources
    6.3 Data Aggregation
    6.4 Performance
    6.5 Help    

7. Left Side Pane
    7.1 Available Data Sources
    7.2 Add Plots
        7.2.1 User Selectable Plot Type Window
        7.2.1 User Selectable Data Source and Context
    7.3 Filters

8. General
    8.1 Application shall be performant - minimize noticeable lag when scrolling/panning/zooming
        8.1.1 Application shall provide time estimation modal in case of large operation
        8.1.2 Application shall provide a 'cancel operation' button to immediately halt an operation if it takes too long
    8.2 Application shall support Ctrl+Z/Ctrl+Y for Undo and Redo
        8.2.1 Undo shall revert to previous application state, with the exception of data added by stream/playback 


---

## Backlog / Feature Ideas

> Ideas that aren't formal requirements yet. Collected here for consideration during future roadmap planning.

- **User-selectable themes:** Allow users to switch between named color/layout themes (e.g. "Engineering Dark", "Light", "High Contrast", "Radar Green"). Theme controls colors, fonts, spacing, and panel proportions. Implementation should use a centralized `AppTheme` struct so themes are swappable at runtime with zero code changes elsewhere.

- **Main Pane Status Bar"** Add a status bar at the bottom of the main window with some stats
- - Loading... receiving, calculating...
- **Rename Plot ID/Titles** Add option to reneame plot Titles to window or in left pane card

- **Polish** Add different graphics to items [A] instead of A for text 0/1 for bools, etc

- **Print/Export Graphics**

- **Loading multiple files** naming them internally, and add a reference to filename (to color plots with)


## Bugs to fix
- Cant see slider in the Performance dropdown
- Fix window stacking/performance
- Right Pane (show/edit filters)
- UI elements no separate thread
- Grouping Filters together
- Editing Filters
- Show more, for legend
- Ability to edit colors, particularly on categoricals
- ability to create a filter based on legend selection
- plot scale adjustments; dialog?
- Map control - zoom with shift, zoom box
- Add 'Ok' to 'Apply' and 'Cancel' in dialogs. Make those buttons always visible
- Show scale values when plotting a lot of categoricals
- when value is requested for tooltip, display 'null' or empty field. (Current behavior hides row)
- size input - has min/max - should have option to tie to min/max value or min/max of dataset (default)
- Tooltip - autosize to fit content
- Some categorical selections not visible/available in selection lists

 
- 