// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/interface_lvalues.sv
interface byte_if;
    logic [7:0] seeded = 8'h12;
    logic [7:0] element_data;
    logic [7:0] slice_data;
    logic [7:0] concat_data;
    logic [7:0] pattern_data;
    logic [7:0] lanes [0:1];

    initial begin
        element_data = 8'h00;
        element_data[0] = 1'b1;

        slice_data = 8'h00;
        slice_data[7:4] = 4'ha;

        concat_data = 8'h00;
        {concat_data[7:4], concat_data[3:0]} = 8'hb2;

        pattern_data = 8'h80;
        '{pattern_data[7], pattern_data[0]} = 2'b01;
    end
endinterface

module tb;
    byte_if bus();

    initial begin
        bus.lanes[0] = 8'h34;
        bus.lanes[1] = 8'h56;
        #1;
        if (bus.element_data !== 8'h01 || bus.slice_data !== 8'ha0 ||
            bus.concat_data !== 8'hb2 || bus.pattern_data !== 8'h01 ||
            bus.seeded !== 8'h12 || bus.lanes[0] !== 8'h34 || bus.lanes[1] !== 8'h56)
            $fatal(1, "interface lvalue/initializer mismatch");
        $display("interface=%h,%h,%h,%h seeded=%h lanes=%h,%h",
                 bus.element_data, bus.slice_data, bus.concat_data, bus.pattern_data,
                 bus.seeded, bus.lanes[0], bus.lanes[1]);
        $finish(0);
    end
endmodule
