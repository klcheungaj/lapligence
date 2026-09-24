// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/legacy_op_replacements.sv
// Focal operations belong either to the selected write address, the same
// outer net source/target, or a source-only value path.

typedef struct packed { bit bank; bit lane; } slot_index_t;
typedef struct { logic [7:0] value; logic [7:0] guard; } record_t;
typedef struct packed { logic [7:0] high; logic [7:0] low; } halves_t;
typedef union packed { logic [15:0] word; halves_t halves; } union_t;

module tb;
    logic clk = 1'b0;
    logic choose = 1'b0;

    record_t conditional_fields [0:1] = '{'{value: 8'h12, guard: 8'h21},
                                           '{value: 8'h34, guard: 8'h43}};
    record_t equality_fields [0:1] = '{'{value: 8'h23, guard: 8'h32},
                                       '{value: 8'h45, guard: 8'h54}};
    record_t cast_fields [0:1] = '{'{value: 8'h34, guard: 8'h43},
                                    '{value: 8'h56, guard: 8'h65}};
    record_t pattern_fields [0:1] = '{'{value: 8'h45, guard: 8'h54},
                                       '{value: 8'h67, guard: 8'h76}};

    logic [7:0] process_source_left = 8'h11;
    logic [7:0] process_source_right = 8'h22;
    logic process_select = 1'b0;
    logic [7:0] process_observed;

    logic [6:0] net_source = 7'b0000001;
    logic net_select = 1'b0;
    wire [7:0] conditional_net;

    logic union_select = 1'b0;
    logic [15:0] union_observed;

    function automatic union_t make_left_union();
        make_left_union = union_t'(16'h1234);
    endfunction

    function automatic union_t make_right_union();
        make_right_union = union_t'(16'h5678);
    endfunction

    function automatic logic [15:0] read_union(input union_t value);
        read_union = value.word;
    endfunction

    assign conditional_net[0] = net_select ? conditional_net[2] : conditional_net[1];
    assign conditional_net[7:1] = net_source;

    always_ff @(posedge clk) begin
        conditional_fields[choose ? 1 : 0].value <= 8'ha5;
        equality_fields[choose == 1'b1].value <= 8'hb6;
        cast_fields[slot_index_t'(choose)].value <= 8'hc7;
        pattern_fields[slot_index_t'{1'b0, choose}].value <= 8'hd8;

        process_observed <= process_select ? process_source_right : process_source_left;
    end

    initial begin
        union_observed = read_union(
            union_select ? make_left_union() : make_right_union());
        if (union_observed !== 16'h5678)
            $fatal(1, "union conditional function source low");

        #1;
        if (conditional_fields[0].value !== 8'h12 || conditional_fields[1].value !== 8'h34 ||
            conditional_fields[0].guard !== 8'h21 || conditional_fields[1].guard !== 8'h43 ||
            equality_fields[0].value !== 8'h23 || equality_fields[1].value !== 8'h45 ||
            equality_fields[0].guard !== 8'h32 || equality_fields[1].guard !== 8'h54 ||
            cast_fields[0].value !== 8'h34 || cast_fields[1].value !== 8'h56 ||
            cast_fields[0].guard !== 8'h43 || cast_fields[1].guard !== 8'h65 ||
            pattern_fields[0].value !== 8'h45 || pattern_fields[1].value !== 8'h67 ||
            pattern_fields[0].guard !== 8'h54 || pattern_fields[1].guard !== 8'h76)
            $fatal(1, "selected write initial values");
        if (conditional_net !== 8'h03)
            $fatal(1, "continuous conditional source initial value");

        net_select = 1'b1;
        #1;
        if (conditional_net !== 8'h02)
            $fatal(1, "continuous conditional selector update");
        net_source = 7'b0000010;
        #1;
        if (conditional_net !== 8'h05)
            $fatal(1, "continuous conditional source update");

        clk = 1'b1;
        #1;
        if (conditional_fields[0].value !== 8'ha5 || conditional_fields[1].value !== 8'h34 ||
            conditional_fields[0].guard !== 8'h21 || conditional_fields[1].guard !== 8'h43 ||
            equality_fields[0].value !== 8'hb6 || equality_fields[1].value !== 8'h45 ||
            equality_fields[0].guard !== 8'h32 || equality_fields[1].guard !== 8'h54 ||
            cast_fields[0].value !== 8'hc7 || cast_fields[1].value !== 8'h56 ||
            cast_fields[0].guard !== 8'h43 || cast_fields[1].guard !== 8'h65 ||
            pattern_fields[0].value !== 8'hd8 || pattern_fields[1].value !== 8'h67 ||
            pattern_fields[0].guard !== 8'h54 || pattern_fields[1].guard !== 8'h76 ||
            process_observed !== 8'h11)
            $fatal(1, "selected write low-address NBA readback");

        clk = 1'b0;
        choose = 1'b1;
        process_select = 1'b1;
        union_select = 1'b1;
        union_observed = read_union(
            union_select ? make_left_union() : make_right_union());
        #1;
        if (conditional_fields[0].value !== 8'ha5 || conditional_fields[1].value !== 8'h34 ||
            equality_fields[0].value !== 8'hb6 || equality_fields[1].value !== 8'h45 ||
            cast_fields[0].value !== 8'hc7 || cast_fields[1].value !== 8'h56 ||
            pattern_fields[0].value !== 8'hd8 || pattern_fields[1].value !== 8'h67 ||
            union_observed !== 16'h1234)
            $fatal(1, "pre-second-edge source and target readback");

        clk = 1'b1;
        #1;
        if (conditional_fields[0].value !== 8'ha5 || conditional_fields[1].value !== 8'ha5 ||
            conditional_fields[0].guard !== 8'h21 || conditional_fields[1].guard !== 8'h43 ||
            equality_fields[0].value !== 8'hb6 || equality_fields[1].value !== 8'hb6 ||
            equality_fields[0].guard !== 8'h32 || equality_fields[1].guard !== 8'h54 ||
            cast_fields[0].value !== 8'hc7 || cast_fields[1].value !== 8'hc7 ||
            cast_fields[0].guard !== 8'h43 || cast_fields[1].guard !== 8'h65 ||
            pattern_fields[0].value !== 8'hd8 || pattern_fields[1].value !== 8'hd8 ||
            pattern_fields[0].guard !== 8'h54 || pattern_fields[1].guard !== 8'h76 ||
            process_observed !== 8'h22)
            $fatal(1, "selected write high-address NBA readback");

        $display("conditional=%h,%h equality=%h,%h cast=%h,%h pattern=%h,%h net=%h union=%h,%h source=%h,%h",
            conditional_fields[0].value, conditional_fields[1].value,
            equality_fields[0].value, equality_fields[1].value,
            cast_fields[0].value, cast_fields[1].value,
            pattern_fields[0].value, pattern_fields[1].value,
            conditional_net, 16'h5678, union_observed,
            process_source_left, process_observed);
        $finish(0);
    end
endmodule
