use druid::debug_state::DebugState;
use druid::piet::PietText;
use druid::text::{FontDescriptor, FontFamily};
use druid::{
    BoxConstraints, Data, Env, Event, EventCtx, LayoutCtx, LifeCycle, LifeCycleCtx, PaintCtx,
    Point, Size, UpdateCtx, Widget, WidgetPod,
};
pub struct CustomFontScope<T, W, F> {
    font: Option<FontFamily>,
    child: WidgetPod<T, W>,
    f: Option<F>,
}

impl<T: Data, W: Widget<T>, F: FnOnce(&mut PietText) -> FontFamily> CustomFontScope<T, W, F> {
    pub fn new(child: W, func: F) -> Self {
        Self {
            font: None,
            child: WidgetPod::new(child),
            f: Some(func),
        }
    }
    fn get_font(&mut self, text: &mut PietText) -> &FontFamily {
        if self.font.is_none() {
            self.font.replace((self.f.take().unwrap())(text));
        }
        self.font.as_ref().unwrap()
    }
    fn edit_env(&mut self, text: &mut PietText, env: &Env) -> Env {
        let mut env = env.clone();
        let font = self.get_font(text);
        env.set(
            druid::theme::UI_FONT,
            FontDescriptor::new(font.clone()).with_size(15.0),
        );
        env.set(
            druid::theme::UI_FONT_BOLD,
            FontDescriptor::new(font.clone())
                .with_weight(druid::piet::FontWeight::BOLD)
                .with_size(15.0),
        );
        env.set(
            druid::theme::UI_FONT_ITALIC,
            FontDescriptor::new(font.clone())
                .with_weight(druid::piet::FontWeight::BOLD)
                .with_size(15.0),
        );
        env
    }
}

impl<T: Data, W: Widget<T>, F: FnOnce(&mut PietText) -> FontFamily> Widget<T>
    for CustomFontScope<T, W, F>
{
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
        let new_env = self.edit_env(ctx.text(), env);
        self.child.event(ctx, event, data, &new_env)
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &T, env: &Env) {
        let new_env = self.edit_env(ctx.text(), env);
        self.child.lifecycle(ctx, event, data, &new_env)
    }
    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &T, data: &T, env: &Env) {
        let new_env = self.edit_env(ctx.text(), env);
        self.child.update(ctx, data, &new_env);
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
        bc.debug_check("CustomFontScope");
        let new_env = self.edit_env(ctx.text(), env);
        let size = self.child.layout(ctx, bc, data, &new_env);
        self.child.set_origin(ctx, Point::ORIGIN);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
        let new_env = self.edit_env(ctx.text(), env);
        self.child.paint(ctx, data, &new_env);
    }

    fn debug_state(&self, data: &T) -> DebugState {
        DebugState {
            display_name: self.short_type_name().to_string(),
            children: vec![self.child.widget().debug_state(data)],
            ..Default::default()
        }
    }
}
