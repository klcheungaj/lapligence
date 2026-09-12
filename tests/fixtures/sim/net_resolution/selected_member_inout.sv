// llg-test-fixture: tests/fixtures/sim/net_resolution/selected_member_inout.sv
module child(input wire en, inout wire [7:0] bus);
    assign bus[0] = en;
endmodule
module tb;
    wire [7:0] bus;
    reg en;
    child u(.en(en), .bus(bus));
    initial begin
        en = 1;
        #1 $display("bus=%b/%b", bus[0], bus[7:1]);
        en = 0;
        #1 $display("bus=%b/%b", bus[0], bus[7:1]);
        $finish(0);
    end
endmodule
