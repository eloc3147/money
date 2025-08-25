from tomlkit import aot, array, document, dumps, item, parse  # type: ignore
from tomlkit.items import Array, AoT, Item
from tomlkit.container import Container

from argparse import ArgumentParser
from pathlib import Path


MAIN_ORDER = ("account", "transaction_type", "rule")
ACCOUNT_ORDER = ("name", "source_path")
RULE_ORDER = (
    "transaction_type",
    "category",
    ("ignore",),
    "patterns",
)
TYPE_ORDER = (
    ("mode",),
    ("prefix",),
    ("source_type",),
    "transaction_type",
    ("income",),
    "name_source",
    "accounts",
)


def sort_table(old_table: list[Container], sort_order: tuple[str, ...]) -> AoT:
    new_table = aot()

    for entry in old_table:
        new_entry: dict[str, Item | Container] = {}
        for key in sort_order:
            optional = False
            if isinstance(key, tuple):
                key = key[0]
                optional = True

            try:
                value = entry[key]
            except Exception as err:
                if optional:
                    continue
                else:
                    raise err

            if isinstance(value, Array):
                new_array = array()
                for array_value in sorted(value):  # type: ignore
                    new_array.append(array_value)  # type: ignore

                if len(new_array) > 1:
                    new_array.multiline(True)

                value = new_array

            new_entry[key] = value

        new_table.append(item(new_entry))  # type: ignore

    return new_table


def main(config_file: Path) -> None:
    original_config = parse(config_file.read_text()).value
    new_config = document()

    for key in MAIN_ORDER:
        value: Array | None = original_config.get(key)
        if value is None:
            continue

        match key:
            case "account":
                order = ACCOUNT_ORDER
                sort_key = "name"
            case "rule":
                order = RULE_ORDER
                sort_key = "category"
            case "transaction_type":
                order = TYPE_ORDER
                sort_key = ("prefix", "source_type")

        sorted_elements = sorted(  # type: ignore
            value,
            key=lambda v: v[sort_key] if isinstance(sort_key, str) else v.get(sort_key[0]) or v.get(sort_key[1])  # type: ignore
        )

        try:
            new_config.append(key, sort_table(sorted_elements, order))  # type: ignore
        except Exception as err:
            raise Exception(f"Failed to sort a {key} table") from err

    config_file.write_text(dumps(new_config))


if __name__ == "__main__":
    parser = ArgumentParser(description="Sort a money app config file")
    parser.add_argument("file", type=Path)
    args = parser.parse_args()

    main(args.file)
