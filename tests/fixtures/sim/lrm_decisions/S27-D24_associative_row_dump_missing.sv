// Decision S27-D24: a dump of one element of an associative array of
// unpacked arrays (aa[k]) whose key does not exist reads the element as an
// expression would: a warning is issued and the array's default value is
// written (unknown cells for a four-state element, zeros for a two-state
// one), with the element's own addressing for a start address. The dump
// never creates the element. An index with X or Z bits is read the same
// way (the sim_027 `assoc_rows` fixture shows it).
//
// IEEE 1800-2009 7.8.6 (SystemVerilog-1800-2009.txt L8722-8724):
//   "If an invalid index (i.e., 4-state expression has X's) is used during a
//   read operation or an attempt is made to read a nonexistent entry, then a
//   warning shall be issued; and the default initial value for the array
//   type shall be returned, as shown in Table 7-1."
// IEEE 1800-2009 21.4 (L37251-37254): the element is a memory_name.
//
// 7.8.6 describes the read of an element in an expression; it does not say
// what a memory dump does with a nonexistent entry. llg follows
// industry-standard behaviour and dumps the value that read returns. For an
// unknown index another implementation stops with an error; llg applies the
// 7.8.6 warning and default.
module tb;
  logic [7:0] aa [int][2];
  bit [7:0] ba [int][2];
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
    $readmemh("s27_d24_in.mem", aa[1]);
    // A key that exists is written.
    $writememh("s27_d24_keep.mem", aa[1]);
    show("s27_d24_keep.mem");
    // A missing key writes the default value over an existing file.
    $writememh("s27_d24_keep.mem", aa[2]);
    show("s27_d24_keep.mem");
    // From a start address, and for a two-state element.
    $writememh("s27_d24_start.mem", aa[3], 1);
    show("s27_d24_start.mem");
    $writememh("s27_d24_bit.mem", ba[5]);
    show("s27_d24_bit.mem");
    $display("elements %0d exists2 %0d bit elements %0d", aa.num(), aa.exists(2), ba.num());
    $finish;
  end
endmodule
