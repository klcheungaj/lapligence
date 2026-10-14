// Decision S27-D24: a dump of one element of an associative array of
// unpacked arrays (aa[k]) whose key does not exist, or whose index has X or Z
// bits, is an error: no file is written (an existing file keeps its
// contents, and a file that did not exist is not created) and no element is
// created. A dump never creates an entry.
//
// IEEE 1800-2009 7.8.6 (SystemVerilog-1800-2009.txt L8722-8724):
//   "If an invalid index (i.e., 4-state expression has X's) is used during a
//   read operation or an attempt is made to read a nonexistent entry, then a
//   warning shall be issued; and the default initial value for the array
//   type shall be returned, as shown in Table 7-1."
// IEEE 1800-2009 21.4 (L37251-37254): the element is a memory_name.
//
// 7.8.6 describes the read of an element in an expression; it does not say
// what a memory dump does with a nonexistent entry. Dumping the default
// value would invent data for an entry the design never had, so llg reports
// the error and writes nothing. This is the conservative reading.
module tb;
  logic [7:0] aa [int][2];
  integer k;
  integer f;

  task show(input string name);
    string line;
    integer fd;
    fd = $fopen(name, "r");
    if (fd == 0) begin
      $display("%s: no file", name);
    end else begin
      $display("%s:", name);
      while ($fgets(line, fd) != 0) $write("  %s", line);
      $fclose(fd);
    end
  endtask

  initial begin
    f = $fopen("s27_d24_in.mem", "w");
    $fwrite(f, "0a 0b\n");
    $fclose(f);
    f = $fopen("s27_d24_keep.mem", "w");
    $fwrite(f, "keep\n");
    $fclose(f);
    $readmemh("s27_d24_in.mem", aa[1]);
    // A key that exists is written.
    $writememh("s27_d24_keep.mem", aa[1]);
    show("s27_d24_keep.mem");
    // A missing key leaves an existing file as it is.
    f = $fopen("s27_d24_keep.mem", "w");
    $fwrite(f, "keep\n");
    $fclose(f);
    $writememh("s27_d24_keep.mem", aa[2]);
    show("s27_d24_keep.mem");
    // A missing key and an unknown key create no file.
    $writememh("s27_d24_new.mem", aa[2]);
    k = 'x;
    $writememh("s27_d24_unknown.mem", aa[k]);
    show("s27_d24_new.mem");
    show("s27_d24_unknown.mem");
    $display("elements %0d exists2 %0d", aa.num(), aa.exists(2));
    $finish;
  end
endmodule
