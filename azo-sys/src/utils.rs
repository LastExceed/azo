macro_rules! c_enum {
	($enum_name:ident, $($variant_name:ident = $value:literal),+) => {
		#[repr(transparent)]
		#[derive(Clone, Copy, PartialEq, Eq, Hash)]
		pub struct $enum_name(pub std::ffi::c_long);

		impl $enum_name {
			$(
				pub const $variant_name: Self = Self($value);
			)+
		}
		
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
		
		impl std::fmt::Debug for $enum_name {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				write!(f, "{}::{}({})", stringify!($enum_name), self, self.0)
			}
		}
	}
}