import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../models/entity_schema.dart';
import '../models/object.dart';
import '../providers/sdui_providers.dart';
import '../sdui/value_format.dart';

/// Экран каталога объектов типа `entityType` (СДУИ, ТЗ §11).
///
/// Колонки — первые поля схемы в порядке `order` (метаданные платформы,
/// никаких захардкоженных типов). Тап по строке → форма просмотра, FAB →
/// форма создания.
class CatalogScreen extends ConsumerWidget {
  const CatalogScreen({super.key, required this.entityType});

  final String entityType;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final schemaAsync = ref.watch(schemaProvider(entityType));
    final objectsAsync = ref.watch(objectsProvider(entityType));

    return schemaAsync.when(
      loading: () => _scaffold(context, null,
          const Center(child: CircularProgressIndicator())),
      error: (e, _) => _scaffold(
        context,
        null,
        Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text('Не удалось загрузить схему: $e'),
          ),
        ),
      ),
      data: (schema) {
        final columns = schema.orderedFields.take(5).toList();
        return objectsAsync.when(
          loading: () => _scaffold(
              context,
              schema.entityType.name,
              const Center(child: CircularProgressIndicator())),
          error: (e, _) => _scaffold(
            context,
            schema.entityType.name,
            Center(
              child: Padding(
                padding: const EdgeInsets.all(24),
                child: Text('Не удалось загрузить объекты: $e'),
              ),
            ),
          ),
          data: (objects) => _scaffold(
            context,
            schema.entityType.name,
            RefreshIndicator(
              onRefresh: () async {
                ref.invalidate(objectsProvider(entityType));
                await ref.read(objectsProvider(entityType).future);
              },
              child: objects.isEmpty
                  ? ListView(
                      physics: const AlwaysScrollableScrollPhysics(),
                      children: const [
                        Padding(
                          padding: EdgeInsets.all(32),
                          child: Center(child: Text('Нет объектов')),
                        ),
                      ],
                    )
                  : _catalogTable(context, columns, objects),
            ),
          ),
        );
      },
    );
  }

  Widget _scaffold(BuildContext context, String? title, Widget body) {
    return Scaffold(
      appBar: AppBar(title: Text(title ?? entityType)),
      floatingActionButton: FloatingActionButton(
        tooltip: 'Создать',
        onPressed: () => context.push('/object/$entityType/new'),
        child: const Icon(Icons.add),
      ),
      body: body,
    );
  }

  Widget _catalogTable(
    BuildContext context,
    List<EntityField> columns,
    List<ObjectItem> objects,
  ) {
    if (columns.isEmpty) {
      return ListView.builder(
        itemCount: objects.length,
        itemBuilder: (context, i) => ListTile(
          title: Text(objects[i].id),
          onTap: () => _openObject(context, objects[i]),
        ),
      );
    }
    return SingleChildScrollView(
      child: DataTable(
        columns: [
          for (final c in columns) DataColumn(label: Text(c.label)),
        ],
        rows: [
          for (final obj in objects)
            DataRow(
              onSelectChanged: (_) => _openObject(context, obj),
              cells: [
                for (final c in columns)
                  DataCell(Text(displayValue(c, obj.data[c.code]))),
              ],
            ),
        ],
      ),
    );
  }

  void _openObject(BuildContext context, ObjectItem obj) {
    context.push('/object/$entityType/${obj.id}');
  }
}