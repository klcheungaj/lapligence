// llg-test-fixture: tests/fixtures/sim/net_resolution/port_cycle.sv
module passthrough(input wire i, output wire o);
    assign o = i ? 1'bz : 1'b0;
endmodule
module tb;
    reg a;
    wor w;
    integer events;
    integer count;
    assign w = a;
    passthrough p(.i(w), .o(w));
    always @(w) if (count) events = events + 1;
    initial begin
        count = 0;
        events = 0;
        a = 1'b1;
        #0;
        count = 1;
        a = 1'b0;
        #1;
        a = 1'b0;
        #1;
        a = 1'b0;
        #1;
        $display("cycle=%b/events=%0d", w, events);
        $finish(0);
    end
endmodule
