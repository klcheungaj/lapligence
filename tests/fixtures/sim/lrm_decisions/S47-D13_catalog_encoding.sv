// Decision S47-D13: the catalog declares every packed variable or net as
// `wire` of its width and every real as `real 64`; the elements of a fixed
// unpacked array are variables `mem[0]`, `mem[1]`; the members of an
// unpacked structure are variables of a `module` scope named after it; nets
// merged through inout ports share one identifier code.
//
// IEEE 1800-2009 21.7.2.1 (SystemVerilog-1800-2009.txt L38066): "Strength
// information and memories are not dumped." (llg dumps array elements as
// separate variables.) Syntax 21-20 (L38010-38015) offers only the scope
// types begin, fork, function, module and task. 23.3.3.7 (L41595-41597): "It
// is permissible to merge the dominating and dominated nets into a single
// net".
// Compare S47-D13.vcd: `mem$5B0$5D`, `$scope module rec`, and tb.bus and
// tb.s.p with one code.
typedef struct {
  logic [3:0] f;
  logic g;
} rec_t;

module side (inout wire p);
  assign p = 1'bz;
endmodule

module tb;
  logic [7:0] mem [0:1];
  rec_t rec;
  wire bus;
  side s(.p(bus));
  assign bus = 1'b1;
  initial begin
    mem[0] = 8'h01;
    mem[1] = 8'h02;
    rec.f = 4'h3;
    rec.g = 1'b1;
    $dumpfile("S47-D13.vcd");
    $dumpvars(0, tb);
    #1 $display("mem=%h,%h rec=%h,%b bus=%b", mem[0], mem[1], rec.f, rec.g, bus);
    $finish;
  end
endmodule
