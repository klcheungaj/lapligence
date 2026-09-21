// llg-test-fixture: tests/fixtures/sim/net_resolution/hierarchical_child_driver.v
// IEEE 1364-2001 3.7.2, 6.1, 12.4: a hierarchical continuous driver
// contributes independently to a child wired net that already has a local
// driver. The equivalent SystemVerilog-2009 source is checked beside this
// fixture.
module wired_child;
    reg local_and;
    reg local_or;
    wand and_net;
    wor or_net;

    assign and_net = local_and;
    assign or_net = local_or;
endmodule

module tb;
    reg parent_and;
    reg parent_or;
    wired_child u();

    assign u.and_net = parent_and;
    assign u.or_net = parent_or;

    initial begin
        parent_and = 1'b1;
        parent_or = 1'b0;
        u.local_and = 1'b0;
        u.local_or = 1'b1;
        #1 $display("CHECK: and=%b or=%b", u.and_net, u.or_net);
        parent_and = 1'bz;
        parent_or = 1'bz;
        #1 $display("CHECK: and=%b or=%b", u.and_net, u.or_net);
        u.local_and = 1'b1;
        u.local_or = 1'b0;
        #1 $display("CHECK: and=%b or=%b", u.and_net, u.or_net);
        $finish(0);
    end
endmodule
