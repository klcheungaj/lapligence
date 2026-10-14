// Decision S27-D22: a whole associative array whose elements are unpacked
// arrays is a memory_name. An address (an @ word of the file, start_addr,
// finish_addr) names the highest dimension, which is the key; it is cast to
// the index type (S27-D16). The words after it fill that element's cells in
// row-major order, starting at its first cell; when the last cell is filled
// the key moves on as in S27-D15 (upward, or toward the finish key), and an
// @ word starts the first cell of its key. A load creates each element it
// reaches with the default value of its type, so a last row the file does
// not fill keeps default cells. A dump writes each existing element as its
// @ key (S27-D17) followed by its cells, one per line, in index order.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37257-37258):
//   "The start_addr and finish_addr arguments apply to the addresses of the
//   unpacked array selected by memory_name. This address range represents
//   the highest dimension of data in the filename."
// IEEE 1800-2009 21.4.3 (L37377-37381):
//   "The file contents are organized in row-major order, with each
//   dimension's entries ranging from low to high address. ... the lowest
//   dimension (i.e., the rightmost dimension in the array declaration)
//   varies the most rapidly."
// IEEE 1800-2009 21.4.1 (L37359-37360):
//   "Loading an address creates an element of that index, if it does not
//   already exist."
// IEEE 1800-2009 21.5.3 (L37500-37501):
//   "When $writememb and $writememh write out data corresponding to
//   associative arrays, address specifiers shall be written to the output
//   file."
//
// The text does not say how an address of an associative array of arrays
// splits into a key and a cell, or where the address specifiers of a dump
// go; reading the key as the highest dimension and writing one @ key per
// element is an llg choice.
module tb;
  logic [7:0] aa [int][2];

  task make(input string name, input string text);
    integer f;
    f = $fopen(name, "w");
    $fwrite(f, "%s", text);
    $fclose(f);
  endtask

  task show(input string name);
    string line;
    integer f;
    f = $fopen(name, "r");
    $display("%s:", name);
    while ($fgets(line, f) != 0) $write("  %s", line);
    $fclose(f);
  endtask

  initial begin
    // Keys 0 and 1 from the default start, then keys 9 and 10 after @9;
    // the third word of each run begins the next element, which the file
    // leaves partly unfilled.
    make("s27_d22_in.mem", "11 12 13\n@9\n21 22 23\n");
    $readmemh("s27_d22_in.mem", aa);
    $writememh("s27_d22_all.mem", aa);
    show("s27_d22_all.mem");
    // A start and finish key bound the elements loaded; the fifth word has
    // no element left.
    make("s27_d22_two.mem", "31 32 33 34 35\n");
    $readmemh("s27_d22_two.mem", aa, 20, 21);
    $readmemh("s27_d22_two.mem", aa, 31, 30);
    $writememh("s27_d22_up.mem", aa, 20, 21);
    show("s27_d22_up.mem");
    $writememh("s27_d22_down.mem", aa, 31, 30);
    show("s27_d22_down.mem");
    $writememh("s27_d22_from.mem", aa, 10);
    show("s27_d22_from.mem");
    $display("elements %0d", aa.num());
    $finish;
  end
endmodule
