macro_rules! c_enum {
	(
		$(#[$struct_attr:meta])*
		$enum_name:ident,
		$(
			$(#[$variant_attr:meta])*
			$variant_name:ident = $value:literal
		),+
	) => {
		#[repr(transparent)]
		#[derive(Clone, Copy, PartialEq, Eq, Hash)]
		$(#[$struct_attr])*
		pub struct $enum_name(pub std::ffi::c_long);

		impl $enum_name {
			$(
				$(#[$variant_attr])*
				pub const $variant_name: Self = Self($value);
			)+
		}
		
		#[automatically_derived]
		impl std::fmt::Display for $enum_name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				let name = match *self {
					$(
						Self::$variant_name => stringify!($variant_name),
					)+
					_ => "<unknown>"
				};
				
				f.write_str(name)
			}
		}
		
		#[automatically_derived]
		impl std::fmt::Debug for $enum_name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				write!(f, "{}::{}({})", stringify!($enum_name), self, self.0)
			}
		}
		
		#[automatically_derived]
		impl From<std::ffi::c_long> for $enum_name {
			fn from(value: std::ffi::c_long) -> Self {
				Self(value)
			}
		}
		
		#[automatically_derived]
		impl From<$enum_name> for std::ffi::c_long {
			fn from(value: $enum_name) -> Self {
				value.0
			}
		}
	}
}