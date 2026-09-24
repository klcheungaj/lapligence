// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/latch_record.sv
// IEEE 1800-2009 §§7.2, 7.4, 9.2.2.3, and 23.3.3: an unpacked record input
// crosses a child port and a record-valued conditional feeds a partial latch
// assignment; a fixed array of records exercises the selected-element path.
package latch_record_pkg;
    typedef struct {
        logic [3:0] high;
        logic [3:0] low;
    } cell_t;

    typedef struct {
        cell_t first;
        cell_t second;
        logic [1:0] kind;
    } source_t;

    typedef struct {
        cell_t data;
        cell_t untouched;
        logic [1:0] kind;
    } result_t;

    typedef cell_t record_array_t [0:1];
endpackage

module record_latch_child(
    input latch_record_pkg::source_t source,
    input logic enable,
    input logic choose_first,
    output latch_record_pkg::result_t held
);
    always_latch begin
        if (enable)
            held.data = choose_first ? source.first : source.second;
    end
endmodule

module array_record_latch_child(
    input latch_record_pkg::record_array_t source,
    input logic enable,
    input logic choose_first,
    input logic selected,
    output latch_record_pkg::record_array_t held
);
    always_latch begin
        if (enable)
            held[selected] = choose_first ? source[0] : source[1];
    end
endmodule

module tb;
    latch_record_pkg::source_t source;
    latch_record_pkg::result_t record_held;
    latch_record_pkg::record_array_t array_source;
    latch_record_pkg::record_array_t array_held;
    logic record_enable;
    logic record_choose_first;
    logic array_enable;
    logic array_choose_first;
    logic array_selected;

    record_latch_child record_dut(
        .source(source),
        .enable(record_enable),
        .choose_first(record_choose_first),
        .held(record_held)
    );
    array_record_latch_child array_dut(
        .source(array_source),
        .enable(array_enable),
        .choose_first(array_choose_first),
        .selected(array_selected),
        .held(array_held)
    );

    initial begin
        source.first.high = 4'h1;
        source.first.low = 4'h2;
        source.second.high = 4'h3;
        source.second.low = 4'h4;
        source.kind = 2'b01;
        record_enable = 1'b0;
        record_choose_first = 1'b0;

        array_source[0].high = 4'h5;
        array_source[0].low = 4'h6;
        array_source[1].high = 4'h7;
        array_source[1].low = 4'h8;
        array_enable = 1'b0;
        array_choose_first = 1'b0;
        array_selected = 1'b1;

        #1;
        $display("closed record=%h,%h array=%h,%h:%h,%h",
                 record_held.data.high, record_held.data.low,
                 array_held[0].high, array_held[0].low,
                 array_held[1].high, array_held[1].low);

        record_enable = 1'b1;
        array_enable = 1'b1;
        #1;
        $display("open record=%h,%h array=%h,%h:%h,%h",
                 record_held.data.high, record_held.data.low,
                 array_held[0].high, array_held[0].low,
                 array_held[1].high, array_held[1].low);

        record_enable = 1'b0;
        array_enable = 1'b0;
        source.first.high = 4'h9;
        source.first.low = 4'ha;
        source.second.high = 4'hb;
        source.second.low = 4'hc;
        record_choose_first = 1'b1;
        array_source[0].high = 4'h9;
        array_source[0].low = 4'ha;
        array_source[1].high = 4'hb;
        array_source[1].low = 4'hc;
        array_choose_first = 1'b1;
        array_selected = 1'b0;
        #1;
        $display("held record=%h,%h array=%h,%h:%h,%h",
                 record_held.data.high, record_held.data.low,
                 array_held[0].high, array_held[0].low,
                 array_held[1].high, array_held[1].low);

        record_enable = 1'b1;
        array_enable = 1'b1;
        #1;
        $display("selected record=%h,%h array=%h,%h:%h,%h",
                 record_held.data.high, record_held.data.low,
                 array_held[0].high, array_held[0].low,
                 array_held[1].high, array_held[1].low);

        source.first.high = 4'hd;
        source.first.low = 4'he;
        array_source[0].high = 4'hd;
        array_source[0].low = 4'he;
        #1;
        $display("updated record=%h,%h array=%h,%h:%h,%h",
                 record_held.data.high, record_held.data.low,
                 array_held[0].high, array_held[0].low,
                 array_held[1].high, array_held[1].low);
        $finish(0);
    end
endmodule
