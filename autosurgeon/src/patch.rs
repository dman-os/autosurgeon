// NOTE: beware of off by one, patches don't include the root
// at index 0.
// i.e. empty patch path is pointing to the root object

mod test {
    use super::*;
    struct Foo {
        string: String,
        integer: i64,
        maybe_string: Option<String>,
    }
    impl Patch for Foo {
        fn apply(
            &mut self,
            self_path_index: usize,
            patch: &automerge::Patch,
        ) -> Result<(), PatchError> {
            if is_patch_on_self(self_path_index, patch)? {
                match &patch.action {
                    automerge::PatchAction::PutMap {
                        key,
                        value: (value, _),
                        conflict: has_conflict,
                    } => match key.as_ref() {
                        "string" => self.string.leaf_apply(value, *has_conflict)?,
                        "integer" => self.integer.leaf_apply(value, *has_conflict)?,
                        "maybe_string" => leaf_apply_option_from_value(
                            &mut self.maybe_string,
                            value,
                            *has_conflict,
                        )?,
                        _ => Err(PathMismatchError::PropMismatch {
                            path_index: self_path_index + 1,
                        })?,
                    },
                    automerge::PatchAction::DeleteMap { key } => match key.as_ref() {
                        "maybe_string" => {
                            let _ = self.maybe_string.take();
                        }
                        "string" | "integer" => Err(LeafApplyError::NotNullable)?,
                        _ => Err(PathMismatchError::PropMismatch {
                            path_index: self_path_index + 1,
                        })?,
                    },
                    _ => Err(PatchError::OpMismatch)?,
                };
            } else {
                Err(PathMismatchError::TooDeep {
                    end_depth: self_path_index,
                })?;
            }
            Ok(())
        }
    }

    struct Bar {
        foo: Foo,
    }
    impl Patch for Bar {
        fn apply(
            &mut self,
            self_path_index: usize,
            patch: &automerge::Patch,
        ) -> Result<(), PatchError> {
            if is_patch_on_self(self_path_index, patch)? {
                Err(PathMismatchError::EOP)?;
            } else {
                match &patch.path[self_path_index].1 {
                    automerge::Prop::Seq(_) => Err(PathMismatchError::PropKindMismatch {
                        path_index: self_path_index,
                    })?,
                    automerge::Prop::Map(prop) => match prop.as_ref() {
                        "foo" => self.foo.apply(self_path_index + 1, patch)?,
                        _ => Err(PathMismatchError::PropMismatch {
                            path_index: self_path_index + 1,
                        })?,
                    },
                }
            }
            Ok(())
        }
    }

    struct Baz {
        bar: Bar,
    }
    impl Patch for Baz {
        fn apply(
            &mut self,
            self_path_index: usize,
            patch: &automerge::Patch,
        ) -> Result<(), PatchError> {
            if is_patch_on_self(self_path_index, patch)? {
                Err(PathMismatchError::EOP)?;
            } else {
                match &patch.path[self_path_index].1 {
                    automerge::Prop::Seq(_) => Err(PathMismatchError::PropKindMismatch {
                        path_index: self_path_index,
                    })?,
                    automerge::Prop::Map(prop) => match prop.as_ref() {
                        "bar" => self.bar.apply(self_path_index + 1, patch)?,
                        _ => Err(PathMismatchError::PropMismatch {
                            path_index: self_path_index + 1,
                        })?,
                    },
                }
            }
            Ok(())
        }
    }

    #[test]
    fn foo_bar_baz() -> Result<(), Box<dyn std::error::Error>> {
        let mut baz = Baz {
            bar: Bar {
                foo: Foo {
                    string: "init".to_string(),
                    integer: 0,
                    maybe_string: Some("hey".into()),
                },
            },
        };

        let obj_id = automerge::ObjId::Root;
        let patches = vec![
            automerge::Patch {
                obj: obj_id.clone(),
                path: vec![
                    (obj_id.clone(), automerge::Prop::Map("bar".to_string())),
                    (obj_id.clone(), automerge::Prop::Map("foo".to_string())),
                ],
                action: automerge::PatchAction::PutMap {
                    key: "string".to_string(),
                    value: (
                        automerge::Value::Scalar(std::borrow::Cow::Owned("new".into())),
                        obj_id.clone(),
                    ),
                    conflict: false,
                },
            },
            automerge::Patch {
                obj: obj_id.clone(),
                path: vec![
                    (obj_id.clone(), automerge::Prop::Map("bar".to_string())),
                    (obj_id.clone(), automerge::Prop::Map("foo".to_string())),
                ],
                action: automerge::PatchAction::PutMap {
                    key: "integer".to_string(),
                    value: (
                        automerge::Value::Scalar(std::borrow::Cow::Owned(
                            automerge::ScalarValue::Int(1),
                        )),
                        obj_id.clone(),
                    ),
                    conflict: false,
                },
            },
            automerge::Patch {
                obj: obj_id.clone(),
                path: vec![
                    (obj_id.clone(), automerge::Prop::Map("bar".to_string())),
                    (obj_id.clone(), automerge::Prop::Map("foo".to_string())),
                ],
                action: automerge::PatchAction::DeleteMap {
                    key: "maybe_string".to_string(),
                },
            },
        ];

        for patch in patches {
            baz.apply(0, &patch)
                .map_err(|err| format!("error applying patch: {err} {patch:?}"))?;
        }

        assert_eq!(baz.bar.foo.string, "new");
        assert_eq!(baz.bar.foo.integer, 1);
        assert_eq!(baz.bar.foo.maybe_string, None);

        Ok(())
    }
}

pub fn path_matches(
    listener_path: &[crate::Prop<'_>],
    change_path: &[(automerge::ObjId, automerge::Prop)],
) -> bool {
    if listener_path.len() > change_path.len() {
        return false;
    }

    for (i, listener_prop) in listener_path.iter().enumerate() {
        if !prop_matches(listener_prop, &change_path[i].1) {
            return false;
        }
    }
    true
}
pub fn prop_matches(listener_prop: &crate::Prop<'_>, change_prop: &automerge::Prop) -> bool {
    match (listener_prop, change_prop) {
        (crate::Prop::Key(listener_key), automerge::Prop::Map(change_key)) => {
            listener_key == change_key
        }
        (crate::Prop::Index(listener_idx), automerge::Prop::Seq(change_idx)) => {
            *listener_idx == (*change_idx as u32)
        }
        _ => false,
    }
}

pub fn is_patch_on_self(
    self_path_index: usize,
    patch: &automerge::Patch,
) -> Result<bool, PathMismatchError> {
    // since our path doesn't include the root,
    // the target object will have index of path.len
    if self_path_index > patch.path.len() {
        Err(PathMismatchError::EOP)
    } else {
        Ok(patch.path.len() == self_path_index)
    }
}

pub fn assert_patch_on_self(
    self_path_index: usize,
    patch: &automerge::Patch,
) -> Result<(), PathMismatchError> {
    if is_patch_on_self(self_path_index, patch)? {
        Ok(())
    } else {
        Err(PathMismatchError::TooDeep {
            end_depth: self_path_index,
        })
    }
}

// TODO: would love to figure out a way to
// make this work with interior mutability and
// Arc/Rc
pub trait Patch {
    fn apply(&mut self, self_path_index: usize, patch: &automerge::Patch)
    -> Result<(), PatchError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutomergeTypeKinds {
    Bytes,
    Str,
    Int,
    Uint,
    F64,
    Counter,
    Timestamp,
    Boolean,
    Unknown,
    Null,
    Map,
    List,
    Text,
}

#[derive(Debug, thiserror::Error)]
pub enum PathMismatchError {
    #[error("unexpected end of path reached while diving through tree")]
    EOP,
    #[error("followed path only has {end_depth:?} nodes, shorter than path")]
    TooDeep { end_depth: usize },
    #[error("unexpected prop kind at index {path_index:?}")]
    PropKindMismatch { path_index: usize },
    #[error("unrecognized prop at index {path_index:?}")]
    PropMismatch { path_index: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum PatchError {
    #[error("path mismatch: {inner:?}")]
    PathMismatch {
        #[from]
        inner: PathMismatchError,
    },
    #[error("error patching leaf: {inner:?}")]
    Leaf {
        #[from]
        inner: LeafApplyError,
    },
    #[error("op mismatch")]
    OpMismatch,
}

pub trait LeafApply {
    // FIXME: this only seems to return TypeMismatch errors
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        // we can use has_conflict to impl a generic wrapper type that indicates conflict state
        has_conflict: bool,
    ) -> Result<(), LeafApplyError>;
}

#[derive(Debug, thiserror::Error)]
pub enum LeafApplyError {
    #[error("type mismatch on value {value:?} expected {expected_kind:?}")]
    ValueTypeMismatch {
        value: automerge::Value<'static>,
        expected_kind: AutomergeTypeKinds,
    },
    #[error("nullable type mismatch")]
    NotNullable,
}

impl LeafApply for String {
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        _has_conflict: bool,
    ) -> Result<(), LeafApplyError> {
        match value {
            automerge::Value::Scalar(cow) if cow.is_str() => match cow.as_ref() {
                automerge::ScalarValue::Str(smol_str) => {
                    self.clear();
                    self.push_str(&smol_str);
                    Ok(())
                }
                _ => unreachable!(),
            },
            value => Err(LeafApplyError::ValueTypeMismatch {
                value: value.to_owned(),
                expected_kind: AutomergeTypeKinds::Str,
            }),
        }
    }
}

// TODO: make judgment call on i32 support with additional cast error
// TODO: feature flag based usize support
impl LeafApply for i64 {
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        _has_conflict: bool,
    ) -> Result<(), LeafApplyError> {
        match value {
            automerge::Value::Scalar(cow) if cow.is_int() => match cow.as_ref() {
                automerge::ScalarValue::Int(int) => {
                    *self = *int;
                    Ok(())
                }
                _ => unreachable!(),
            },
            value => Err(LeafApplyError::ValueTypeMismatch {
                value: value.to_owned(),
                expected_kind: AutomergeTypeKinds::Str,
            }),
        }
    }
}

impl<T> Patch for Box<T>
where
    T: Patch,
{
    fn apply(
        &mut self,
        self_path_index: usize,
        patch: &automerge::Patch,
    ) -> Result<(), PatchError> {
        T::apply(self, self_path_index, patch)
    }
}

impl<T> LeafApply for Box<T>
where
    T: LeafApply,
{
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        has_conflict: bool,
    ) -> Result<(), LeafApplyError> {
        T::leaf_apply(&mut *self, value, has_conflict)
    }
}

impl<T> Patch for std::sync::RwLock<T>
where
    T: Patch,
{
    fn apply(
        &mut self,
        self_path_index: usize,
        patch: &automerge::Patch,
    ) -> Result<(), PatchError> {
        let mut guard = self.write().unwrap();
        T::apply(&mut guard, self_path_index, patch)
    }
}

impl<T> LeafApply for std::sync::RwLock<T>
where
    T: LeafApply,
{
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        has_conflict: bool,
    ) -> Result<(), LeafApplyError> {
        let mut guard = self.write().unwrap();
        guard.leaf_apply(value, has_conflict)
    }
}

impl<T> Patch for std::sync::Mutex<T>
where
    T: Patch,
{
    fn apply(
        &mut self,
        self_path_index: usize,
        patch: &automerge::Patch,
    ) -> Result<(), PatchError> {
        let mut guard = self.lock().unwrap();
        T::apply(&mut guard, self_path_index, patch)
    }
}

impl<T> LeafApply for std::sync::Mutex<T>
where
    T: LeafApply,
{
    fn leaf_apply(
        &mut self,
        value: &automerge::Value<'_>,
        has_conflict: bool,
    ) -> Result<(), LeafApplyError> {
        let mut guard = self.lock().unwrap();
        guard.leaf_apply(value, has_conflict)
    }
}

trait FromValue: Sized {
    fn from_value(value: &automerge::Value<'_>, has_conflict: bool)
    -> Result<Self, LeafApplyError>;
}

impl FromValue for String {
    fn from_value(
        value: &automerge::Value<'_>,
        _has_conflict: bool,
    ) -> Result<Self, LeafApplyError> {
        match value {
            automerge::Value::Scalar(cow) if cow.is_str() => match cow.as_ref() {
                automerge::ScalarValue::Str(smol_str) => Ok(smol_str.to_string()),
                _ => unreachable!(),
            },
            value => Err(LeafApplyError::ValueTypeMismatch {
                value: value.to_owned(),
                expected_kind: AutomergeTypeKinds::Str,
            }),
        }
    }
}
impl FromValue for i64 {
    fn from_value(
        value: &automerge::Value<'_>,
        _has_conflict: bool,
    ) -> Result<Self, LeafApplyError> {
        match value {
            automerge::Value::Scalar(cow) if cow.is_int() => match cow.as_ref() {
                automerge::ScalarValue::Int(int) => Ok(*int),
                _ => unreachable!(),
            },
            value => Err(LeafApplyError::ValueTypeMismatch {
                value: value.to_owned(),
                expected_kind: AutomergeTypeKinds::Str,
            }),
        }
    }
}

// TODO: proc macro attribute for default value
impl<T> Patch for Option<T>
where
    T: Patch + Default,
{
    fn apply(
        &mut self,
        self_path_index: usize,
        patch: &automerge::Patch,
    ) -> Result<(), PatchError> {
        match self {
            Some(inner) => inner.apply(self_path_index, patch),
            None => {
                let mut inner = T::default();
                inner.apply(self_path_index, patch)?;
                self.replace(inner);
                Ok(())
            }
        }
    }
}

/// TODO: proc macro attribute to choose between default and from_value
pub fn leaf_apply_option_default<T>(
    this: &mut Option<T>,
    value: &automerge::Value<'_>,
    has_conflict: bool,
    default_fn: impl FnOnce() -> T,
) -> Result<(), LeafApplyError>
where
    T: LeafApply,
{
    match this {
        Some(inner) => inner.leaf_apply(value, has_conflict),
        None => {
            let mut inner = default_fn();
            inner.leaf_apply(value, has_conflict)?;
            this.replace(inner);
            Ok(())
        }
    }
}

pub fn leaf_apply_option_from_value<T>(
    this: &mut Option<T>,
    value: &automerge::Value<'_>,
    has_conflict: bool,
) -> Result<(), LeafApplyError>
where
    T: LeafApply + FromValue,
{
    match this {
        Some(inner) => inner.leaf_apply(value, has_conflict),
        None => {
            this.replace(T::from_value(value, has_conflict)?);
            Ok(())
        }
    }
}
