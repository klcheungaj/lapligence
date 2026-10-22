// Decision S27-D25: the dump of one element of an associative array of
// unpacked arrays (aa[k]), with or without an index or slice inside it,
// writes the cells of a fixed unpacked array: one per line, lowest address
// first, with no address specifiers (the whole associative array is not a
// memory_name, S27-D22). A descending element dimension is written from its
// lowest index, like a fixed array (S27-D11).
//
// IEEE 1800-2009 21.5.3 (SystemVerilog-1800-2009.txt L37497-37501):
//   "When $writememb and $writememh write out data corresponding to unpacked
//   or dynamic arrays, address specifiers (@-words) shall not be written to
//   the output file. When $writememb and $writememh write out data
//   corresponding to associative arrays, address specifiers shall be
//   written to the output file."
// IEEE 1800-2009 21.4 (L37251-37254): aa[k] resolves to a lesser dimensioned
// unpacked array.
// IEEE 1800-2009 21.4.3 (L37377-37381): "each dimension's entries ranging
//   from low to high address."
//
// Whether the data of aa[k] counts as "associative array" data or as the
// data of the unpacked array it resolves to is not stated; llg takes the
// resolved unpacked array, which has no index to write.
module tb;
  logic [7:0] aa [int][1:0];

  task show(input string name);
    string line;
    integer fd;
    fd = $fopen(name, "r");
    $display("%s:", name);
    while ($fgets(line, fd) != 0) $write("  %s", line);
    $fclose(fd);
  endtask

  integer f;
  initial begin
    f = $fopen("s27_d25_in.mem", "w");
    $fwrite(f, "0a 0b\n");
    $fclose(f);
    $readmemh("s27_d25_in.mem", aa[3]);
    $readmemh("s27_d25_in.mem", aa[4], 1);
    $writememh("s27_d25_one.mem", aa[3]);
    show("s27_d25_one.mem");
    $writememh("s27_d25_cell.mem", aa[3][1:1]);
    show("s27_d25_cell.mem");
    $writememh("s27_d25_start.mem", aa[3], 1);
    show("s27_d25_start.mem");
    $writememh("s27_d25_four.mem", aa[4]);
    show("s27_d25_four.mem");
    $finish;
  end
endmodule
