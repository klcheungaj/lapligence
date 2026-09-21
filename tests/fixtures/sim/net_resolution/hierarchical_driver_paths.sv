// llg-test-fixture: tests/fixtures/sim/net_resolution/hierarchical_driver_paths.sv
// IEEE 1800-2009 6.6, 10.3, 23.6: selected, generated and upward-qualified
// hierarchical continuous drivers keep independent wired-net contributions.
module net_child;
    reg and_value;
    reg or_value;
    wand [3:0] and_bus;
    wor [3:0] or_bus;

    assign and_bus = {4{and_value}};
    assign or_bus = {4{or_value}};
endmodule

module up_source;
    reg value;

    // The path climbs to the parent and then descends through a sibling
    // instance. Slang diagnoses the upward reference, while the simulator
    // preserves the legal continuous driver site.
    assign tb.downstream.and_bus[0] = value;
endmodule

module tb;
    reg [3:0] parent;
    net_child u();
    net_child downstream();
    up_source source();
    genvar i;
    generate
        for (i = 0; i < 1; i = i + 1) begin : g
            net_child generated();
        end
    endgenerate

    assign u.and_bus[3:2] = parent[3:2];
    assign u.or_bus[1:0] = parent[1:0];
    assign g[0].generated.and_bus[0] = parent[0];

    initial begin
        parent = 4'b1010;
        u.and_value = 1'b1;
        u.or_value = 1'b0;
        downstream.and_value = 1'b0;
        downstream.or_value = 1'b0;
        g[0].generated.and_value = 1'b0;
        g[0].generated.or_value = 1'b0;
        source.value = 1'b1;
        #1 $display("CHECK: first=%b/%b/%b/%b", u.and_bus, u.or_bus,
                    g[0].generated.and_bus, downstream.and_bus);

        parent = 4'bzzzz;
        g[0].generated.and_value = 1'b1;
        downstream.and_value = 1'b1;
        source.value = 1'b0;
        #1 $display("CHECK: second=%b/%b/%b/%b", u.and_bus, u.or_bus,
                    g[0].generated.and_bus, downstream.and_bus);
        $finish(0);
    end
endmodule
