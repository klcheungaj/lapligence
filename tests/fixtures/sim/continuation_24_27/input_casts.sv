// llg-test-fixture: fixed input values retain each nested cast (SV 6.24, 7.6, 23.3.3).
`ifndef CONTINUATION_PORT_W
`define CONTINUATION_PORT_W 65
`endif
module cast_sink #(parameter W = 65) (
    input logic [W-1:0] value [5:6],
    output wire [2*W-1:0] observed
);
    assign observed = {value[5], value[6]};
endmodule
module tb;
    localparam W = `CONTINUATION_PORT_W;
    typedef logic [W-1:0] lane_t;
    typedef lane_t row_t [2:1];
    typedef bit [W-1:0] bit_lane_t;
    typedef bit_lane_t bit_row_t [7:8];
    typedef logic [2*W-1:0] packed_t;
    row_t source;
    row_t assigned;
    wire [2*W-1:0] converted, unchanged, packed_cast;
    cast_sink #(W) casted(.value(row_t'(bit_row_t'(source))), .observed(converted));
    cast_sink #(W) control(.value(source), .observed(unchanged));
    assign packed_cast = packed_t'(bit_row_t'(source));

    task automatic check_values;
        for (int n = 0; n < 2; n++) begin
            for (int b = 0; b < W; b++) begin
                if (converted[(1-n)*W+b] !== (source[2-n][b] === 1'b1))
                    $fatal(1, "input cast lost intermediate state conversion");
                if (packed_cast[(1-n)*W+b] !== (source[2-n][b] === 1'b1))
                    $fatal(1, "packed cast bypassed the converted array value");
                if (assigned[2-n][b] !== (source[2-n][b] === 1'b1))
                    $fatal(1, "assignment scatter bypassed intermediate cast");
            end
        end
        if (unchanged !== {source[2], source[1]}) $fatal(1, "uncast input changed");
    endtask
    initial begin
        source[2] = 'x;
        source[1] = 'z;
        source[2][0] = 1;
        assigned = row_t'(bit_row_t'(source));
        #1; check_values();
        source[1][W-1] = 1;
        assigned = row_t'(bit_row_t'(source));
        #1; check_values();
        source[2] = '1;
        source[1] = '0;
        assigned = row_t'(bit_row_t'(source));
        #1; check_values();
        $display("INPUT_CASTS_PASS W=%0d", W);
        $finish(0);
    end
endmodule
