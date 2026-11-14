import QtQuick
import qs.Commons
import qs.Ui

// OpenStreetMap slippy map for pinning the online location. Tiles come from the backend,
// which downloads them with Dulook's User-Agent and caches them (OSM tile policy).
// Drag to pan, wheel or +/− to zoom, click to drop the pin.
Item {
  id: map
  clip: true

  property var service: null
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property real centerLatitude: 20
  property real centerLongitude: 0
  property int zoom: 2
  property real pinLatitude: NaN
  property real pinLongitude: NaN

  signal picked(real latitude, real longitude)

  readonly property int minZoom: 2
  readonly property int maxZoom: 19
  readonly property int tileSize: 256
  readonly property real worldSize: tileSize * Math.pow(2, zoom)
  readonly property real centerX: lonToX(centerLongitude)
  readonly property real centerY: latToY(centerLatitude)
  // Changes only when another tile comes into view, so delegates are not rebuilt while dragging.
  readonly property string tileRange: {
    var last = Math.pow(2, zoom) - 1
    var x0 = Math.floor((centerX - width / 2) / tileSize)
    var x1 = Math.floor((centerX + width / 2) / tileSize)
    var y0 = Math.max(0, Math.floor((centerY - height / 2) / tileSize))
    var y1 = Math.min(last, Math.floor((centerY + height / 2) / tileSize))
    return [zoom, x0, x1, y0, y1].join("/")
  }
  readonly property var tiles: {
    var range = tileRange.split("/").map(Number)
    var n = Math.pow(2, range[0])
    var result = []
    for (var ty = range[3]; ty <= range[4]; ty++)
      for (var tx = range[1]; tx <= range[2]; tx++)
        result.push({ key: range[0] + "/" + (((tx % n) + n) % n) + "/" + ty, tx: tx, ty: ty })
    return result
  }
  readonly property var readyTiles: {
    var ready = {}
    var list = service ? service.mapTiles : []
    for (var i = 0; i < list.length; i++) ready[list[i]] = true
    return ready
  }

  function clampZoom(value) { return Math.max(minZoom, Math.min(maxZoom, value)) }
  function lonToX(lon) { return (lon + 180) / 360 * worldSize }
  function latToY(lat) {
    var s = Math.sin(Math.max(-85.05112878, Math.min(85.05112878, lat)) * Math.PI / 180)
    return (0.5 - Math.log((1 + s) / (1 - s)) / (4 * Math.PI)) * worldSize
  }
  function xToLon(x) {
    var lon = x / worldSize * 360 - 180
    return ((lon + 180) % 360 + 360) % 360 - 180
  }
  function yToLat(y) {
    var t = Math.PI - 2 * Math.PI * y / worldSize
    return Math.atan(0.5 * (Math.exp(t) - Math.exp(-t))) * 180 / Math.PI
  }

  onTilesChanged: tileTimer.restart()
  onVisibleChanged: if (visible) tileTimer.restart()

  Timer {
    id: tileTimer
    interval: 150
    onTriggered: if (map.visible && map.service) map.service.requestMapTiles(map.tiles.map(function(tile) { return tile.key }))
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.rgba(map.foreground.r, map.foreground.g, map.foreground.b, 0.06)
  }

  Repeater {
    model: map.tiles
    Image {
      required property var modelData
      x: modelData.tx * map.tileSize - (map.centerX - map.width / 2)
      y: modelData.ty * map.tileSize - (map.centerY - map.height / 2)
      width: map.tileSize
      height: map.tileSize
      smooth: false
      asynchronous: true
      source: map.readyTiles[modelData.key] && map.service
        ? "file://" + map.service.mapTileDir + "/" + modelData.key + ".png" : ""
    }
  }

  Rectangle {
    visible: !isNaN(map.pinLatitude) && !isNaN(map.pinLongitude)
    width: 14
    height: 14
    radius: 7
    x: map.width / 2 + map.lonToX(map.pinLongitude) - map.centerX - width / 2
    y: map.height / 2 + map.latToY(map.pinLatitude) - map.centerY - height / 2
    color: Color.urgent
    border.width: 2
    border.color: "white"
  }

  MouseArea {
    id: mouse
    anchors.fill: parent
    // The settings Flickable would otherwise take over the drag.
    preventStealing: true
    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor

    property real pressX: 0
    property real pressY: 0
    property real startX: 0
    property real startY: 0
    property bool dragged: false
    property real wheelDelta: 0

    onPressed: function(event) {
      pressX = event.x
      pressY = event.y
      startX = map.centerX
      startY = map.centerY
      dragged = false
    }
    onPositionChanged: function(event) {
      var dx = event.x - pressX
      var dy = event.y - pressY
      if (!dragged && Math.abs(dx) + Math.abs(dy) <= 4) return
      dragged = true
      map.centerLongitude = map.xToLon(startX - dx)
      map.centerLatitude = map.yToLat(Math.max(0, Math.min(map.worldSize, startY - dy)))
    }
    onReleased: function(event) {
      if (dragged) return
      var px = map.centerX - map.width / 2 + event.x
      var py = map.centerY - map.height / 2 + event.y
      if (py >= 0 && py <= map.worldSize) map.picked(map.yToLat(py), map.xToLon(px))
    }
    // One zoom level per wheel notch; touchpads send many small steps.
    onWheel: function(event) {
      wheelDelta += event.angleDelta.y
      while (Math.abs(wheelDelta) >= 120) {
        map.zoom = map.clampZoom(map.zoom + (wheelDelta > 0 ? 1 : -1))
        wheelDelta -= wheelDelta > 0 ? 120 : -120
      }
      event.accepted = true
    }
  }

  Column {
    anchors.top: parent.top
    anchors.right: parent.right
    anchors.margins: Style.spacing.sm
    spacing: Style.spacing.sm
    Button {
      text: "+"
      bordered: true
      foreground: map.foreground
      background: Color.background
      fontFamily: map.fontFamily
      enabled: map.zoom < map.maxZoom
      onClicked: map.zoom = map.clampZoom(map.zoom + 1)
    }
    Button {
      text: "−"
      bordered: true
      foreground: map.foreground
      background: Color.background
      fontFamily: map.fontFamily
      enabled: map.zoom > map.minZoom
      onClicked: map.zoom = map.clampZoom(map.zoom - 1)
    }
  }

  // OSM requires visible attribution.
  Rectangle {
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    width: attribution.implicitWidth + 2 * Style.spacing.sm
    height: attribution.implicitHeight + Style.spacing.sm
    color: Qt.rgba(0, 0, 0, 0.45)
    Text {
      id: attribution
      anchors.centerIn: parent
      text: map.service ? map.service.tr("mapAttribution") : ""
      textFormat: Text.PlainText
      color: "white"
      font.family: map.fontFamily
      font.pixelSize: Style.font.caption
    }
  }
}
