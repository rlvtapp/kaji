@0xa0dcbe745b242510;
using Common = import "common.capnp";
interface Store { get @0 (item :Common.Item) -> (item :Common.Item); }
