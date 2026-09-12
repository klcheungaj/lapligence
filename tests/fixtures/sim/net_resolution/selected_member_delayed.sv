// llg-test-fixture: tests/fixtures/sim/net_resolution/selected_member_delayed.sv
`timescale 1ns/1ns
module child(input wire en, inout wire [7:0] bus);
    assign #2 bus[0] = en ? 1'b1 : 1'bz;
endmodule
module tb;
    wire [7:0] bus;
    reg en;
    child u(.en(en), .bus(bus));
    initial begin
        en = 0;
        #1 en = 1;
        #1 if (bus[0] !== 1'bz || bus[7:1] !== 7'bzzzzzzz) $display("FAIL before_delay=%b", bus);
        #1 if (bus[0] !== 1'b1 || bus[7:1] !== 7'bzzzzzzz) $display("FAIL after_rise=%b", bus);
        en = 0;
        #1 if (bus[0] !== 1'b1 || bus[7:1] !== 7'bzzzzzzz) $display("FAIL before_fall=%b", bus);
        #2 if (bus[0] !== 1'bz || bus[7:1] !== 7'bzzzzzzz) $display("FAIL after_fall=%b", bus);
        $display("PASS selected_member_delayed");
        $finish(0);
    end
endmodule
