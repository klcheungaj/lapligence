// Decision S27-D26: X and Z bits of a start or finish address of $readmem,
// $sreadmem and $writemem read as 0.
//
// IEEE 1800-2009 21.4 (SystemVerilog-1800-2009.txt L37257-37258):
//   "The start_addr and finish_addr arguments apply to the addresses of the
//   unpacked array selected by memory_name." Neither 21.4, 21.5 nor D.14
//   says what an address with unknown bits selects.
//
// llg follows industry-standard behaviour: the unknown bits are taken as 0,
// so 4'bx selects address 0 and 4'b01x0 selects address 4, for loads from
// files and strings, for dumps and for associative memories. The call
// otherwise follows the rules for a known address.
module tb;
  logic [7:0] mem [0:7];
  logic [7:0] aa [int];
  logic [3:0] unknown = 4'bx;
  logic [3:0] high_z = 4'bz;
  logic [3:0] partial = 4'b01x0;
  integer fd, f, c;
  string line;
  task fresh;
    foreach (mem[i]) mem[i] = 8'h5a;
  endtask
  task show(string tag);
    $display("%s %h %h %h %h %h %h %h %h", tag, mem[0], mem[1], mem[2], mem[3], mem[4],
             mem[5], mem[6], mem[7]);
  endtask
  initial begin
    fd = $fopen("s27_d26.mem", "w");
    $fwrite(fd, "11 22 33\n");
    $fclose(fd);
    fresh; $sreadmemh(mem, unknown, 2, "11 22 33"); show("string x start");
    fresh; $sreadmemh(mem, high_z, 2, "11 22 33"); show("string z start");
    fresh; $sreadmemh(mem, partial, 6, "11 22 33"); show("string partial");
    fresh; $readmemh("s27_d26.mem", mem, 2, unknown); show("file x finish");
    fresh; $readmemh("s27_d26.mem", mem, partial); show("file partial");
    foreach (mem[i]) mem[i] = 8'(i);
    $writememh("s27_d26_dump.mem", mem, 2, unknown);
    f = $fopen("s27_d26_dump.mem", "r");
    $write("dump:");
    while ($fgets(line, f) != 0) $write(" %s", line.substr(0, line.len() - 2));
    $write("\n");
    $fclose(f);
    $readmemh("s27_d26.mem", aa, partial, 6);
    foreach (aa[k]) $display("aa[%0d]=%h", k, aa[k]);
    $finish;
  end
endmodule
