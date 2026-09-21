// llg-test-fixture: tests/fixtures/sim/net_resolution/hierarchical_r05_driver.sv
// IEEE 1364-2001 12.3.10 and IEEE 1800-2009 23.3, 23.6: an independent
// hierarchical driver may target a connected inout group after R05 chooses
// the effective resolver for dissimilar wand/wor declarations.
module leaf(inout wand p);
    assign p = 1'bz;
endmodule

module middle(inout wor p);
    leaf inner(p);
endmodule

module tb;
    wire bus;
    middle outer(bus);

    assign outer.inner.p = 1'b1;

    initial begin
        #1 $display("CHECK: bus=%b p=%b", bus, outer.inner.p);
        $finish(0);
    end
endmodule
