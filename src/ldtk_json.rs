// ------------------------------------
// Auto-generated by quicktype from the LDtk 1.5.2 minimal JSON schema
// ------------------------------------

use serde::{Deserialize, Serialize};

/// This file is a JSON schema of files created by LDtk level editor (https://ldtk.io).
///
/// This is the root of any Project JSON file. It contains:  - the project settings, - an
/// array of levels, - a group of definitions (that can probably be safely ignored for most
/// users).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// This object is not actually used by LDtk. It ONLY exists to force explicit references to
    /// all types, to make sure QuickType finds them and integrate all of them. Otherwise,
    /// Quicktype will drop types that are not explicitely used.
    #[serde(rename = "__FORCED_REFS")]
    pub forced_refs: Option<ForcedRefs>,

    /// Project background color
    pub bg_color: String,

    /// A structure containing all the definitions of this project
    pub defs: Definitions,

    /// If TRUE, one file will be saved for the project (incl. all its definitions) and one file
    /// in a sub-folder for each level.
    pub external_levels: bool,

    /// Unique project identifier
    pub iid: String,

    /// File format version
    pub json_version: String,

    /// All levels. The order of this array is only relevant in `LinearHorizontal` and
    /// `linearVertical` world layouts (see `worldLayout` value).<br/>  Otherwise, you should
    /// refer to the `worldX`,`worldY` coordinates of each Level.
    pub levels: Vec<Level>,

    /// All instances of entities that have their `exportToToc` flag enabled are listed in this
    /// array.
    pub toc: Vec<LdtkTableOfContentEntry>,

    /// **WARNING**: this field will move to the `worlds` array after the "multi-worlds" update.
    /// It will then be `null`. You can enable the Multi-worlds advanced project option to enable
    /// the change immediately.<br/><br/>  Height of the world grid in pixels.
    pub world_grid_height: Option<i64>,

    /// **WARNING**: this field will move to the `worlds` array after the "multi-worlds" update.
    /// It will then be `null`. You can enable the Multi-worlds advanced project option to enable
    /// the change immediately.<br/><br/>  Width of the world grid in pixels.
    pub world_grid_width: Option<i64>,

    /// **WARNING**: this field will move to the `worlds` array after the "multi-worlds" update.
    /// It will then be `null`. You can enable the Multi-worlds advanced project option to enable
    /// the change immediately.<br/><br/>  An enum that describes how levels are organized in
    /// this project (ie. linearly or in a 2D space). Possible values: &lt;`null`&gt;, `Free`,
    /// `GridVania`, `LinearHorizontal`, `LinearVertical`, `null`
    pub world_layout: Option<WorldLayout>,

    /// This array will be empty, unless you enable the Multi-Worlds in the project advanced
    /// settings.<br/><br/> - in current version, a LDtk project file can only contain a single
    /// world with multiple levels in it. In this case, levels and world layout related settings
    /// are stored in the root of the JSON.<br/> - with "Multi-worlds" enabled, there will be a
    /// `worlds` array in root, each world containing levels and layout settings. Basically, it's
    /// pretty much only about moving the `levels` array to the `worlds` array, along with world
    /// layout related values (eg. `worldGridWidth` etc).<br/><br/>If you want to start
    /// supporting this future update easily, please refer to this documentation:
    /// https://github.com/deepnight/ldtk/issues/231
    pub worlds: Vec<World>,
}

/// If you're writing your own LDtk importer, you should probably just ignore *most* stuff in
/// the `defs` section, as it contains data that are mostly important to the editor. To keep
/// you away from the `defs` section and avoid some unnecessary JSON parsing, important data
/// from definitions is often duplicated in fields prefixed with a double underscore (eg.
/// `__identifier` or `__type`).  The 2 only definition types you might need here are
/// **Tilesets** and **Enums**.
///
/// A structure containing all the definitions of this project
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Definitions {
    /// All entities definitions, including their custom fields
    pub entities: Vec<EntityDefinition>,

    /// All internal enums
    pub enums: Vec<EnumDefinition>,

    /// Note: external enums are exactly the same as `enums`, except they have a `relPath` to
    /// point to an external source file.
    pub external_enums: Vec<EnumDefinition>,

    /// All layer definitions
    pub layers: Vec<LayerDefinition>,

    /// All custom fields available to all levels.
    pub level_fields: Vec<FieldDefinition>,

    /// All tilesets
    pub tilesets: Vec<TilesetDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityDefinition {
    /// Base entity color
    pub color: String,

    /// Pixel height
    pub height: i64,

    /// User defined unique identifier
    pub identifier: String,

    /// An array of 4 dimensions for the up/right/down/left borders (in this order) when using
    /// 9-slice mode for `tileRenderMode`.<br/>  If the tileRenderMode is not NineSlice, then
    /// this array is empty.<br/>  See: https://en.wikipedia.org/wiki/9-slice_scaling
    pub nine_slice_borders: Vec<i64>,

    /// Pivot X coordinate (from 0 to 1.0)
    pub pivot_x: f64,

    /// Pivot Y coordinate (from 0 to 1.0)
    pub pivot_y: f64,

    /// An object representing a rectangle from an existing Tileset
    pub tile_rect: Option<TilesetRectangle>,

    /// An enum describing how the the Entity tile is rendered inside the Entity bounds. Possible
    /// values: `Cover`, `FitInside`, `Repeat`, `Stretch`, `FullSizeCropped`,
    /// `FullSizeUncropped`, `NineSlice`
    pub tile_render_mode: TileRenderMode,

    /// Tileset ID used for optional tile display
    pub tileset_id: Option<i64>,

    /// Unique Int identifier
    pub uid: i64,

    /// This tile overrides the one defined in `tileRect` in the UI
    pub ui_tile_rect: Option<TilesetRectangle>,

    /// Pixel width
    pub width: i64,
}

/// This object represents a custom sub rectangle in a Tileset image.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TilesetRectangle {
    /// Height in pixels
    pub h: i64,

    /// UID of the tileset
    pub tileset_uid: i64,

    /// Width in pixels
    pub w: i64,

    /// X pixels coordinate of the top-left corner in the Tileset image
    pub x: i64,

    /// Y pixels coordinate of the top-left corner in the Tileset image
    pub y: i64,
}

/// An enum describing how the the Entity tile is rendered inside the Entity bounds. Possible
/// values: `Cover`, `FitInside`, `Repeat`, `Stretch`, `FullSizeCropped`,
/// `FullSizeUncropped`, `NineSlice`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TileRenderMode {
    Cover,

    #[serde(rename = "FitInside")]
    FitInside,

    #[serde(rename = "FullSizeCropped")]
    FullSizeCropped,

    #[serde(rename = "FullSizeUncropped")]
    FullSizeUncropped,

    #[serde(rename = "NineSlice")]
    NineSlice,

    Repeat,

    Stretch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumDefinition {
    /// Relative path to the external file providing this Enum
    pub external_rel_path: Option<String>,

    /// Tileset UID if provided
    pub icon_tileset_uid: Option<i64>,

    /// User defined unique identifier
    pub identifier: String,

    /// An array of user-defined tags to organize the Enums
    pub tags: Vec<String>,

    /// Unique Int identifier
    pub uid: i64,

    /// All possible enum values, with their optional Tile infos.
    pub values: Vec<EnumValueDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumValueDefinition {
    /// Optional color
    pub color: i64,

    /// Enum value
    pub id: String,

    /// Optional tileset rectangle to represents this value
    pub tile_rect: Option<TilesetRectangle>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerDefinition {
    /// Type of the layer (*IntGrid, Entities, Tiles or AutoLayer*)
    #[serde(rename = "__type")]
    pub layer_definition_type: String,

    pub auto_source_layer_def_uid: Option<i64>,

    /// Opacity of the layer (0 to 1.0)
    pub display_opacity: f64,

    /// Width and height of the grid in pixels
    pub grid_size: i64,

    /// User defined unique identifier
    pub identifier: String,

    /// An array that defines extra optional info for each IntGrid value.<br/>  WARNING: the
    /// array order is not related to actual IntGrid values! As user can re-order IntGrid values
    /// freely, you may value "2" before value "1" in this array.
    pub int_grid_values: Vec<IntGridValueDefinition>,

    /// Group informations for IntGrid values
    pub int_grid_values_groups: Vec<IntGridValueGroupDefinition>,

    /// Parallax horizontal factor (from -1 to 1, defaults to 0) which affects the scrolling
    /// speed of this layer, creating a fake 3D (parallax) effect.
    pub parallax_factor_x: f64,

    /// Parallax vertical factor (from -1 to 1, defaults to 0) which affects the scrolling speed
    /// of this layer, creating a fake 3D (parallax) effect.
    pub parallax_factor_y: f64,

    /// If true (default), a layer with a parallax factor will also be scaled up/down accordingly.
    pub parallax_scaling: bool,

    /// X offset of the layer, in pixels (IMPORTANT: this should be added to the `LayerInstance`
    /// optional offset)
    pub px_offset_x: i64,

    /// Y offset of the layer, in pixels (IMPORTANT: this should be added to the `LayerInstance`
    /// optional offset)
    pub px_offset_y: i64,

    /// Reference to the default Tileset UID being used by this layer definition.<br/>
    /// **WARNING**: some layer *instances* might use a different tileset. So most of the time,
    /// you should probably use the `__tilesetDefUid` value found in layer instances.<br/>  Note:
    /// since version 1.0.0, the old `autoTilesetDefUid` was removed and merged into this value.
    pub tileset_def_uid: Option<i64>,

    /// Unique Int identifier
    pub uid: i64,
}

/// IntGrid value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntGridValueDefinition {
    pub color: String,

    /// Parent group identifier (0 if none)
    pub group_uid: i64,

    /// User defined unique identifier
    pub identifier: Option<String>,

    pub tile: Option<TilesetRectangle>,

    /// The IntGrid value itself
    pub value: i64,
}

/// IntGrid value group definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntGridValueGroupDefinition {
    /// User defined color
    pub color: Option<String>,

    /// User defined string identifier
    pub identifier: Option<String>,

    /// Group unique ID
    pub uid: i64,
}

/// This section is mostly only intended for the LDtk editor app itself. You can safely
/// ignore it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {}

/// The `Tileset` definition is the most important part among project definitions. It
/// contains some extra informations about each integrated tileset. If you only had to parse
/// one definition section, that would be the one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TilesetDefinition {
    /// Grid-based height
    #[serde(rename = "__cHei")]
    pub c_hei: i64,

    /// Grid-based width
    #[serde(rename = "__cWid")]
    pub c_wid: i64,

    /// An array of custom tile metadata
    pub custom_data: Vec<TileCustomMetadata>,

    /// If this value is set, then it means that this atlas uses an internal LDtk atlas image
    /// instead of a loaded one. Possible values: &lt;`null`&gt;, `LdtkIcons`, `null`
    pub embed_atlas: Option<EmbedAtlas>,

    /// Tileset tags using Enum values specified by `tagsSourceEnumId`. This array contains 1
    /// element per Enum value, which contains an array of all Tile IDs that are tagged with it.
    pub enum_tags: Vec<EnumTagValue>,

    /// User defined unique identifier
    pub identifier: String,

    /// Distance in pixels from image borders
    pub padding: i64,

    /// Image height in pixels
    pub px_hei: i64,

    /// Image width in pixels
    pub px_wid: i64,

    /// Path to the source file, relative to the current project JSON file<br/>  It can be null
    /// if no image was provided, or when using an embed atlas.
    pub rel_path: Option<String>,

    /// Space in pixels between all tiles
    pub spacing: i64,

    /// An array of user-defined tags to organize the Tilesets
    pub tags: Vec<String>,

    /// Optional Enum definition UID used for this tileset meta-data
    pub tags_source_enum_uid: Option<i64>,

    pub tile_grid_size: i64,

    /// Unique Intidentifier
    pub uid: i64,
}

/// In a tileset definition, user defined meta-data of a tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileCustomMetadata {
    pub data: String,

    pub tile_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbedAtlas {
    #[serde(rename = "LdtkIcons")]
    LdtkIcons,
}

/// In a tileset definition, enum based tag infos
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumTagValue {
    pub enum_value_id: String,

    pub tile_ids: Vec<i64>,
}

/// This object is not actually used by LDtk. It ONLY exists to force explicit references to
/// all types, to make sure QuickType finds them and integrate all of them. Otherwise,
/// Quicktype will drop types that are not explicitely used.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ForcedRefs {
    pub auto_layer_rule_group: Option<AutoLayerRuleGroup>,

    pub auto_rule_def: Option<AutoLayerRuleDefinition>,

    pub custom_command: Option<LdtkCustomCommand>,

    pub definitions: Option<Definitions>,

    pub entity_def: Option<EntityDefinition>,

    pub entity_instance: Option<EntityInstance>,

    pub entity_reference_infos: Option<ReferenceToAnEntityInstance>,

    pub enum_def: Option<EnumDefinition>,

    pub enum_def_values: Option<EnumValueDefinition>,

    pub enum_tag_value: Option<EnumTagValue>,

    pub field_def: Option<FieldDefinition>,

    pub field_instance: Option<FieldInstance>,

    pub grid_point: Option<GridPoint>,

    pub int_grid_value_def: Option<IntGridValueDefinition>,

    pub int_grid_value_group_def: Option<IntGridValueGroupDefinition>,

    pub int_grid_value_instance: Option<IntGridValueInstance>,

    pub layer_def: Option<LayerDefinition>,

    pub layer_instance: Option<LayerInstance>,

    pub level: Option<Level>,

    pub level_bg_pos_infos: Option<LevelBackgroundPosition>,

    pub neighbour_level: Option<NeighbourLevel>,

    pub table_of_content_entry: Option<LdtkTableOfContentEntry>,

    pub tile: Option<TileInstance>,

    pub tile_custom_metadata: Option<TileCustomMetadata>,

    pub tileset_def: Option<TilesetDefinition>,

    pub tileset_rect: Option<TilesetRectangle>,

    pub world: Option<World>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoLayerRuleGroup {
    pub active: bool,

    pub color: Option<String>,

    pub icon: Option<TilesetRectangle>,

    pub is_optional: bool,

    pub name: String,

    pub rules: Vec<AutoLayerRuleDefinition>,

    pub uid: i64,

    pub uses_wizard: bool,
}

/// This complex section isn't meant to be used by game devs at all, as these rules are
/// completely resolved internally by the editor before any saving. You should just ignore
/// this part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoLayerRuleDefinition {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdtkCustomCommand {
    pub command: String,

    /// Possible values: `Manual`, `AfterLoad`, `BeforeSave`, `AfterSave`
    pub when: When,
}

/// Possible values: `Manual`, `AfterLoad`, `BeforeSave`, `AfterSave`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum When {
    #[serde(rename = "AfterLoad")]
    AfterLoad,

    #[serde(rename = "AfterSave")]
    AfterSave,

    #[serde(rename = "BeforeSave")]
    BeforeSave,

    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityInstance {
    /// Grid-based coordinates (`[x,y]` format)
    #[serde(rename = "__grid")]
    pub grid: Vec<i64>,

    /// Entity definition identifier
    #[serde(rename = "__identifier")]
    pub identifier: String,

    /// Pivot coordinates  (`[x,y]` format, values are from 0 to 1) of the Entity
    #[serde(rename = "__pivot")]
    pub pivot: Vec<f64>,

    /// The entity "smart" color, guessed from either Entity definition, or one its field
    /// instances.
    #[serde(rename = "__smartColor")]
    pub smart_color: String,

    /// Array of tags defined in this Entity definition
    #[serde(rename = "__tags")]
    pub tags: Vec<String>,

    /// Optional TilesetRect used to display this entity (it could either be the default Entity
    /// tile, or some tile provided by a field value, like an Enum).
    #[serde(rename = "__tile")]
    pub tile: Option<TilesetRectangle>,

    /// X world coordinate in pixels
    #[serde(rename = "__worldX")]
    pub world_x: i64,

    /// Y world coordinate in pixels
    #[serde(rename = "__worldY")]
    pub world_y: i64,

    /// Reference of the **Entity definition** UID
    pub def_uid: i64,

    /// An array of all custom fields and their values.
    pub field_instances: Vec<FieldInstance>,

    /// Entity height in pixels. For non-resizable entities, it will be the same as Entity
    /// definition.
    pub height: i64,

    /// Unique instance identifier
    pub iid: String,

    /// Pixel coordinates (`[x,y]` format) in current level coordinate space. Don't forget
    /// optional layer offsets, if they exist!
    pub px: Vec<i64>,

    /// Entity width in pixels. For non-resizable entities, it will be the same as Entity
    /// definition.
    pub width: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInstance {
    /// Field definition identifier
    #[serde(rename = "__identifier")]
    pub identifier: String,

    /// Optional TilesetRect used to display this field (this can be the field own Tile, or some
    /// other Tile guessed from the value, like an Enum).
    #[serde(rename = "__tile")]
    pub tile: Option<TilesetRectangle>,

    /// Type of the field, such as `Int`, `Float`, `String`, `Enum(my_enum_name)`, `Bool`,
    /// etc.<br/>  NOTE: if you enable the advanced option **Use Multilines type**, you will have
    /// "*Multilines*" instead of "*String*" when relevant.
    #[serde(rename = "__type")]
    pub field_instance_type: String,

    /// Actual value of the field instance. The value type varies, depending on `__type`:<br/>
    /// - For **classic types** (ie. Integer, Float, Boolean, String, Text and FilePath), you
    /// just get the actual value with the expected type.<br/>   - For **Color**, the value is an
    /// hexadecimal string using "#rrggbb" format.<br/>   - For **Enum**, the value is a String
    /// representing the selected enum value.<br/>   - For **Point**, the value is a
    /// [GridPoint](#ldtk-GridPoint) object.<br/>   - For **Tile**, the value is a
    /// [TilesetRect](#ldtk-TilesetRect) object.<br/>   - For **EntityRef**, the value is an
    /// [EntityReferenceInfos](#ldtk-EntityReferenceInfos) object.<br/><br/>  If the field is an
    /// array, then this `__value` will also be a JSON array.
    #[serde(rename = "__value")]
    pub value: Option<serde_json::Value>,

    /// Reference of the **Field definition** UID
    pub def_uid: i64,
}

/// This object describes the "location" of an Entity instance in the project worlds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceToAnEntityInstance {
    /// IID of the refered EntityInstance
    pub entity_iid: String,

    /// IID of the LayerInstance containing the refered EntityInstance
    pub layer_iid: String,

    /// IID of the Level containing the refered EntityInstance
    pub level_iid: String,

    /// IID of the World containing the refered EntityInstance
    pub world_iid: String,
}

/// This object is just a grid-based coordinate used in Field values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPoint {
    /// X grid-based coordinate
    pub cx: i64,

    /// Y grid-based coordinate
    pub cy: i64,
}

/// IntGrid value instance
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntGridValueInstance {
    /// Coordinate ID in the layer grid
    pub coord_id: i64,

    /// IntGrid value
    pub v: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerInstance {
    /// Grid-based height
    #[serde(rename = "__cHei")]
    pub c_hei: i64,

    /// Grid-based width
    #[serde(rename = "__cWid")]
    pub c_wid: i64,

    /// Grid size
    #[serde(rename = "__gridSize")]
    pub grid_size: i64,

    /// Layer definition identifier
    #[serde(rename = "__identifier")]
    pub identifier: String,

    /// Layer opacity as Float [0-1]
    #[serde(rename = "__opacity")]
    pub opacity: f64,

    /// Total layer X pixel offset, including both instance and definition offsets.
    #[serde(rename = "__pxTotalOffsetX")]
    pub px_total_offset_x: i64,

    /// Total layer Y pixel offset, including both instance and definition offsets.
    #[serde(rename = "__pxTotalOffsetY")]
    pub px_total_offset_y: i64,

    /// The definition UID of corresponding Tileset, if any.
    #[serde(rename = "__tilesetDefUid")]
    pub tileset_def_uid: Option<i64>,

    /// The relative path to corresponding Tileset, if any.
    #[serde(rename = "__tilesetRelPath")]
    pub tileset_rel_path: Option<String>,

    /// Layer type (possible values: IntGrid, Entities, Tiles or AutoLayer)
    #[serde(rename = "__type")]
    pub layer_instance_type: String,

    /// An array containing all tiles generated by Auto-layer rules. The array is already sorted
    /// in display order (ie. 1st tile is beneath 2nd, which is beneath 3rd etc.).<br/><br/>
    /// Note: if multiple tiles are stacked in the same cell as the result of different rules,
    /// all tiles behind opaque ones will be discarded.
    pub auto_layer_tiles: Vec<TileInstance>,

    pub entity_instances: Vec<EntityInstance>,

    pub grid_tiles: Vec<TileInstance>,

    /// Unique layer instance identifier
    pub iid: String,

    /// A list of all values in the IntGrid layer, stored in CSV format (Comma Separated
    /// Values).<br/>  Order is from left to right, and top to bottom (ie. first row from left to
    /// right, followed by second row, etc).<br/>  `0` means "empty cell" and IntGrid values
    /// start at 1.<br/>  The array size is `__cWid` x `__cHei` cells.
    pub int_grid_csv: Vec<i64>,

    /// Reference the Layer definition UID
    pub layer_def_uid: i64,

    /// Reference to the UID of the level containing this layer instance
    pub level_id: i64,

    /// This layer can use another tileset by overriding the tileset UID here.
    pub override_tileset_uid: Option<i64>,

    /// X offset in pixels to render this layer, usually 0 (IMPORTANT: this should be added to
    /// the `LayerDef` optional offset, so you should probably prefer using `__pxTotalOffsetX`
    /// which contains the total offset value)
    pub px_offset_x: i64,

    /// Y offset in pixels to render this layer, usually 0 (IMPORTANT: this should be added to
    /// the `LayerDef` optional offset, so you should probably prefer using `__pxTotalOffsetX`
    /// which contains the total offset value)
    pub px_offset_y: i64,

    /// Layer instance visibility
    pub visible: bool,
}

/// This structure represents a single tile from a given Tileset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileInstance {
    /// Alpha/opacity of the tile (0-1, defaults to 1)
    pub a: f64,

    /// "Flip bits", a 2-bits integer to represent the mirror transformations of the tile.<br/>
    /// - Bit 0 = X flip<br/>   - Bit 1 = Y flip<br/>   Examples: f=0 (no flip), f=1 (X flip
    /// only), f=2 (Y flip only), f=3 (both flips)
    pub f: i64,

    /// Pixel coordinates of the tile in the **layer** (`[x,y]` format). Don't forget optional
    /// layer offsets, if they exist!
    pub px: Vec<i64>,

    /// Pixel coordinates of the tile in the **tileset** (`[x,y]` format)
    pub src: Vec<i64>,

    /// The *Tile ID* in the corresponding tileset.
    pub t: i64,
}

/// This section contains all the level data. It can be found in 2 distinct forms, depending
/// on Project current settings:  - If "*Separate level files*" is **disabled** (default):
/// full level data is *embedded* inside the main Project JSON file, - If "*Separate level
/// files*" is **enabled**: level data is stored in *separate* standalone `.ldtkl` files (one
/// per level). In this case, the main Project JSON file will still contain most level data,
/// except heavy sections, like the `layerInstances` array (which will be null). The
/// `externalRelPath` string points to the `ldtkl` file.  A `ldtkl` file is just a JSON file
/// containing exactly what is described below.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    /// Background color of the level (same as `bgColor`, except the default value is
    /// automatically used here if its value is `null`)
    #[serde(rename = "__bgColor")]
    pub bg_color: String,

    /// Position informations of the background image, if there is one.
    #[serde(rename = "__bgPos")]
    pub bg_pos: Option<LevelBackgroundPosition>,

    /// An array listing all other levels touching this one on the world map. Since 1.4.0, this
    /// includes levels that overlap in the same world layer, or in nearby world layers.<br/>
    /// Only relevant for world layouts where level spatial positioning is manual (ie. GridVania,
    /// Free). For Horizontal and Vertical layouts, this array is always empty.
    #[serde(rename = "__neighbours")]
    pub neighbours: Vec<NeighbourLevel>,

    /// The *optional* relative path to the level background image.
    pub bg_rel_path: Option<String>,

    /// This value is not null if the project option "*Save levels separately*" is enabled. In
    /// this case, this **relative** path points to the level Json file.
    pub external_rel_path: Option<String>,

    /// An array containing this level custom field values.
    pub field_instances: Vec<FieldInstance>,

    /// User defined unique identifier
    pub identifier: String,

    /// Unique instance identifier
    pub iid: String,

    /// An array containing all Layer instances. **IMPORTANT**: if the project option "*Save
    /// levels separately*" is enabled, this field will be `null`.<br/>  This array is **sorted
    /// in display order**: the 1st layer is the top-most and the last is behind.
    pub layer_instances: Option<Vec<LayerInstance>>,

    /// Height of the level in pixels
    pub px_hei: i64,

    /// Width of the level in pixels
    pub px_wid: i64,

    /// Unique Int identifier
    pub uid: i64,

    /// Index that represents the "depth" of the level in the world. Default is 0, greater means
    /// "above", lower means "below".<br/>  This value is mostly used for display only and is
    /// intended to make stacking of levels easier to manage.
    pub world_depth: i64,

    /// World X coordinate in pixels.<br/>  Only relevant for world layouts where level spatial
    /// positioning is manual (ie. GridVania, Free). For Horizontal and Vertical layouts, the
    /// value is always -1 here.
    pub world_x: i64,

    /// World Y coordinate in pixels.<br/>  Only relevant for world layouts where level spatial
    /// positioning is manual (ie. GridVania, Free). For Horizontal and Vertical layouts, the
    /// value is always -1 here.
    pub world_y: i64,
}

/// Level background image position info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelBackgroundPosition {
    /// An array of 4 float values describing the cropped sub-rectangle of the displayed
    /// background image. This cropping happens when original is larger than the level bounds.
    /// Array format: `[ cropX, cropY, cropWidth, cropHeight ]`
    pub crop_rect: Vec<f64>,

    /// An array containing the `[scaleX,scaleY]` values of the **cropped** background image,
    /// depending on `bgPos` option.
    pub scale: Vec<f64>,

    /// An array containing the `[x,y]` pixel coordinates of the top-left corner of the
    /// **cropped** background image, depending on `bgPos` option.
    pub top_left_px: Vec<i64>,
}

/// Nearby level info
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NeighbourLevel {
    /// A single lowercase character tipping on the level location (`n`orth, `s`outh, `w`est,
    /// `e`ast).<br/>  Since 1.4.0, this character value can also be `<` (neighbour depth is
    /// lower), `>` (neighbour depth is greater) or `o` (levels overlap and share the same world
    /// depth).
    pub dir: String,

    /// Neighbour Instance Identifier
    pub level_iid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdtkTableOfContentEntry {
    pub identifier: String,

    pub instances: Vec<ReferenceToAnEntityInstance>,
}

/// **IMPORTANT**: this type is available as a preview. You can rely on it to update your
/// importers, for when it will be officially available.  A World contains multiple levels,
/// and it has its own layout settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    /// User defined unique identifier
    pub identifier: String,

    /// Unique instance identifer
    pub iid: String,

    /// All levels from this world. The order of this array is only relevant in
    /// `LinearHorizontal` and `linearVertical` world layouts (see `worldLayout` value).
    /// Otherwise, you should refer to the `worldX`,`worldY` coordinates of each Level.
    pub levels: Vec<Level>,

    /// Height of the world grid in pixels.
    pub world_grid_height: i64,

    /// Width of the world grid in pixels.
    pub world_grid_width: i64,

    /// An enum that describes how levels are organized in this project (ie. linearly or in a 2D
    /// space). Possible values: `Free`, `GridVania`, `LinearHorizontal`, `LinearVertical`,
    /// `null`, `null`
    pub world_layout: Option<WorldLayout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldLayout {
    Free,

    #[serde(rename = "GridVania")]
    GridVania,

    #[serde(rename = "LinearHorizontal")]
    LinearHorizontal,

    #[serde(rename = "LinearVertical")]
    LinearVertical,
}
