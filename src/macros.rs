#[macro_export]
macro_rules! ecs_query {
    ($entities:ident[$name:expr], $($a:ident $($b:ident)?),*) => {
        $entities.get($name).map(|e| Some((
            $( ecs_query!(impl e $a $($b)?), )*
        ))).flatten()
    };

    ($entities:ident, $($a:ident $($b:ident)?),*) => {
        $entities.values().filter_map(|e| Some((
            $( ecs_query!(impl e $a $($b)?), )*
        )))
    };

    (impl $e:ident mut $component:ident) => {
        $crate::utils::refmut_opt_to_opt_refmut($e.$component.borrow_mut())?
    };

    (impl $e:ident $component:ident) => {
        $crate::utils::ref_opt_to_opt_ref($e.$component.borrow())?
    };
}
