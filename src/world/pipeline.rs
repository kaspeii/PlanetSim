use bevy::prelude::*;

/// Гарантирует строгий порядок выполнения в каждом кадре: Ввод -> Симуляция -> Рендер
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum PipelineSet {
    Input,
    Simulation,
    Render,
}

/// Ресурс состояния пайплайна: позволяет UI и хоткеям запрашивать пересчет конкретных стадий
#[derive(Resource)]
pub struct WorldPipeline {
    /// Полная перегенерация тектоники с нуля
    pub regenerate_tectonics: bool,
    /// Полная перегенерация температуры
    pub regenerate_climate: bool,
    /// Перестроение геометрии и цветов 3D/2D меша
    pub rebuild_mesh: bool,
    /// Перекрашивание меша
    pub recolor_mesh: bool,
}

impl Default for WorldPipeline {
    fn default() -> Self {
        Self {
            regenerate_tectonics: true, // Запускаем первичную генерацию на старте
            regenerate_climate: false,
            rebuild_mesh: false,
            recolor_mesh: false,
        }
    }
}
