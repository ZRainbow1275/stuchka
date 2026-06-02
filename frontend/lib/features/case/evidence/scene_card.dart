import 'package:exif/exif.dart';
import 'package:flutter/material.dart';

import '../../../theme/stuchka_icons.dart';
import '../../../theme/stuchka_theme.dart';

/// Real capture-scene metadata extracted from an imported file's bytes (EXIF/GPS for photos).
/// Every field is read from the actual file — when a value is absent the field stays null and the
/// [SceneCard] says so explicitly (an honest "not present", never a fabricated coordinate/time).
class SceneMeta {
  const SceneMeta({
    required this.fileName,
    required this.byteSize,
    this.capturedAt,
    this.latitude,
    this.longitude,
    this.cameraModel,
    this.exifRead = false,
  });

  final String fileName;
  final int byteSize;

  /// EXIF DateTimeOriginal, verbatim (e.g. "2024:03:01 14:22:10"), when present.
  final String? capturedAt;

  /// Decimal-degrees GPS, when the file carries GPS EXIF tags.
  final double? latitude;
  final double? longitude;

  /// EXIF Image Model, when present.
  final String? cameraModel;

  /// Whether an EXIF block was found at all (distinguishes "no EXIF" from "EXIF without GPS/time").
  final bool exifRead;

  bool get hasGps => latitude != null && longitude != null;
}

/// Read EXIF/GPS from the real bytes. Never throws — a non-image / EXIF-less file yields a
/// [SceneMeta] with the file identity only and `exifRead == false`.
Future<SceneMeta> extractScene(String fileName, List<int> bytes) async {
  try {
    final tags = await readExifFromBytes(bytes);
    if (tags.isEmpty) {
      return SceneMeta(fileName: fileName, byteSize: bytes.length, exifRead: false);
    }
    final captured = tags['EXIF DateTimeOriginal']?.printable ??
        tags['Image DateTime']?.printable;
    final model = tags['Image Model']?.printable;
    final lat = _gpsDecimal(tags['GPS GPSLatitude'], tags['GPS GPSLatitudeRef']?.printable);
    final lon = _gpsDecimal(tags['GPS GPSLongitude'], tags['GPS GPSLongitudeRef']?.printable);
    return SceneMeta(
      fileName: fileName,
      byteSize: bytes.length,
      capturedAt: (captured != null && captured.trim().isNotEmpty) ? captured.trim() : null,
      latitude: lat,
      longitude: lon,
      cameraModel: (model != null && model.trim().isNotEmpty) ? model.trim() : null,
      exifRead: true,
    );
  } catch (_) {
    // Malformed / unsupported bytes — surface the file identity only, never crash the import.
    return SceneMeta(fileName: fileName, byteSize: bytes.length, exifRead: false);
  }
}

/// Convert an EXIF GPS coordinate (deg/min/sec ratios) + hemisphere ref into decimal degrees.
double? _gpsDecimal(IfdTag? coord, String? ref) {
  if (coord == null) return null;
  final values = coord.values.toList();
  if (values.length < 3) return null;
  double part(int i) {
    final v = values[i];
    if (v is Ratio) {
      return v.denominator == 0 ? 0.0 : v.numerator / v.denominator;
    }
    return double.tryParse('$v') ?? 0.0;
  }

  final deg = part(0) + part(1) / 60.0 + part(2) / 3600.0;
  final r = (ref ?? '').toUpperCase();
  return (r == 'S' || r == 'W') ? -deg : deg;
}

/// A read-only card surfacing the imported evidence's real scene metadata (file identity + the
/// backend-computed SHA-256 + EXIF/GPS when present). Lucide icons only; zero emoji.
class SceneCard extends StatelessWidget {
  const SceneCard({super.key, required this.scene, this.sha256});

  final SceneMeta scene;
  final String? sha256;

  @override
  Widget build(BuildContext context) {
    final colors = StuchkaSemanticColors.of(context);
    final rows = <Widget>[
      _row(context, '文件', scene.fileName),
      _row(context, '大小', '${(scene.byteSize / 1024).toStringAsFixed(1)} KB'),
      if (sha256 != null && sha256!.isNotEmpty)
        _row(context, 'SHA-256', sha256!, mono: true),
    ];
    if (scene.capturedAt != null) {
      rows.add(_row(context, '拍摄时间 (EXIF)', scene.capturedAt!));
    }
    if (scene.cameraModel != null) {
      rows.add(_row(context, '拍摄设备 (EXIF)', scene.cameraModel!));
    }
    if (scene.hasGps) {
      rows.add(_row(
        context,
        'GPS 坐标 (EXIF)',
        '${scene.latitude!.toStringAsFixed(6)}, ${scene.longitude!.toStringAsFixed(6)}',
      ));
    } else {
      // Honest seam: declare what was not present rather than implying a location.
      rows.add(_row(
        context,
        'GPS 坐标 (EXIF)',
        scene.exifRead ? '该文件 EXIF 中无 GPS 定位信息' : '未在该文件中读取到 EXIF 元数据',
      ));
    }

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(StuchkaIcons.evidence, color: colors.inkBlue, size: 20),
                const SizedBox(width: 8),
                Text('取证信息卡', style: Theme.of(context).textTheme.titleMedium),
              ],
            ),
            const SizedBox(height: 8),
            ...rows,
          ],
        ),
      ),
    );
  }

  Widget _row(BuildContext context, String label, String value, {bool mono = false}) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 120,
            child: Text(label, style: Theme.of(context).textTheme.bodySmall),
          ),
          Expanded(
            child: Text(
              value,
              style: mono
                  ? const TextStyle(fontFamily: 'JetBrainsMonoSC', fontSize: 11)
                  : Theme.of(context).textTheme.bodyMedium,
            ),
          ),
        ],
      ),
    );
  }
}
