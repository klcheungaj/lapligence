// llg-test-fixture: R05 port net-type collapse / selected
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3.3.7.
`timescale 1ns/1ns
module and_leaf(inout wand [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module or_leaf(inout wor [3:0] p, input wire [3:0] d);
    assign p = d;
endmodule
module plain(inout wire [7:0] p); endmodule
module tb;
    wire [7:0] bus;
    reg [7:0] parent_d;
    reg [3:0] and_d, or_d;
    // Whole port precedes selected ports: collection must promote it later.
    plain first(bus);
    and_leaf u0(bus[3:0], and_d);
    or_leaf u1(bus[7:4], or_d);
    assign bus = parent_d;
    initial begin
        parent_d = 8'h55; and_d = 4'ha; or_d = 4'ha;
        #1 $display("selected=%h/%h", bus, first.p);
        force bus[2] = 1;
        #1 $display("force=%h/%h", bus, first.p);
        release bus[2];
        #1 $display("release=%h/%h", bus, first.p);
        parent_d = 8'hxx; and_d = 0; or_d = 4'hf;
        #1 $display("unknown=%h/%h", bus, first.p);
        parent_d = 8'hzz; and_d = 4'hz; or_d = 4'hz;
        #1 $display("float=%h/%h", bus, first.p);
        $finish;
    end
endmodule
