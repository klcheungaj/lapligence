// llg-test-fixture: tests/fixtures/sim/net_resolution/force_selected_inout.sv
module child(input wire en, inout wire [3:0] bus);
    assign bus = en ? 4'b0011 : 4'bzzzz;
endmodule
module tb;
    wire [3:0] bus;
    reg en;
    child u(.en(en), .bus(bus));
    initial begin
        en = 1;
        #1 $display("base=%b", bus);
        force bus[1:0] = 2'b00;
        #1 $display("forced=%b", bus);
        en = 0;
        #1 $display("underlying=%b", bus);
        release bus[0];
        #1 $display("lower_release=%b", bus);
        release bus[1];
        #1 $display("all_release=%b", bus);
        $finish(0);
    end
endmodule
