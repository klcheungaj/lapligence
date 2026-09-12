// IEEE 1800-2009 7.4, 7.6, 7.7, and 10.10: fixed unpacked-array values,
// slices, partial indexing, concatenation, and compatible element conversion.
module tb;
    logic [7:0] source [3:0];
    logic [7:0] target [3:0];
    logic [7:0] slice_source [0:3];
    logic [7:0] slice_target [0:3];
    typedef bit [3:0] bit_lane_t;
    typedef bit_lane_t bit_array_t [0:1];
    logic [3:0] state_source [0:1];
    bit_array_t two_state_target;
    real real_source [0:1];
    real real_target [0:1];
    logic [7:0] nba_source [0:1];
    logic [7:0] nba_target [0:1];
    int selected_row;
    logic [7:0] ascending [0:3];
    logic [7:0] matrix [1:0][0:1];
    logic [7:0] row_copy [0:1];
    logic [7:0] dynamic_values[];

    initial begin
        source = '{8'h11, 8'h22, 8'h33, 8'h44};
        target = source;
        ascending = source;
        if (target[3] !== 8'h11 || target[2] !== 8'h22 ||
            target[1] !== 8'h33 || target[0] !== 8'h44 ||
            ascending[0] !== 8'h11 || ascending[1] !== 8'h22 ||
            ascending[2] !== 8'h33 || ascending[3] !== 8'h44) begin
            $display("FAIL fixed_array_assignment whole_copy");
            $finish;
        end

        matrix = '{'{8'ha1, 8'ha2}, '{8'hb1, 8'hb2}};
        row_copy = matrix[1];
        if (row_copy[0] !== 8'ha1 || row_copy[1] !== 8'ha2) begin
            $display("FAIL fixed_array_assignment partial_index %h %h", row_copy[0], row_copy[1]);
            $finish;
        end
        selected_row = 1;
        row_copy = matrix[selected_row];
        if (row_copy[0] !== 8'ha1 || row_copy[1] !== 8'ha2) begin
            $display("FAIL fixed_array_assignment dynamic_partial_index");
            $finish;
        end

        row_copy = '{8'hc1, 8'hc2};
        matrix[0] = row_copy;
        if (matrix[0][0] !== 8'hc1 || matrix[0][1] !== 8'hc2) begin
            $display("FAIL fixed_array_assignment partial_lvalue");
            $finish;
        end

        matrix[1:0] = '{'{8'hd1, 8'hd2}, '{8'he1, 8'he2}};
        if (matrix[1][0] !== 8'hd1 || matrix[1][1] !== 8'hd2 ||
            matrix[0][0] !== 8'he1 || matrix[0][1] !== 8'he2) begin
            $display("FAIL fixed_array_assignment multidim_slice");
            $finish;
        end

        matrix[1][0:1] = '{8'hf1, 8'hf2};
        if (matrix[1][0] !== 8'hf1 || matrix[1][1] !== 8'hf2) begin
            $display("FAIL fixed_array_assignment nested_slice");
            $finish;
        end
        row_copy = matrix[1][0:1];
        if (row_copy[0] !== 8'hf1 || row_copy[1] !== 8'hf2) begin
            $display("FAIL fixed_array_assignment nested_slice_read");
            $finish;
        end

        dynamic_values = new[4];
        dynamic_values = '{8'h05, 8'h06, 8'h07, 8'h08};
        target = dynamic_values;
        if (target[3] !== 8'h05 || target[2] !== 8'h06 ||
            target[1] !== 8'h07 || target[0] !== 8'h08) begin
            $display("FAIL fixed_array_assignment dynamic_conversion");
            $finish;
        end

        slice_source = '{8'ha1, 8'ha2, 8'ha3, 8'ha4};
        slice_target = '{8'h00, 8'h00, 8'h00, 8'h00};
        slice_target[2:3] = slice_source[0:1];
        if (slice_target[2] !== 8'ha1 || slice_target[3] !== 8'ha2) begin
            $display("FAIL fixed_array_assignment slice %h %h", slice_target[2], slice_target[3]);
            $finish;
        end

        target[2:1] = source[3:2];
        if (target[2] !== source[3] || target[1] !== source[2]) begin
            $display("FAIL fixed_array_assignment descending_slice %h %h", target[2], target[1]);
            $finish;
        end

        slice_target = {slice_source[0:1], slice_source[2:3]};
        if (slice_target[0] !== 8'ha1 || slice_target[1] !== 8'ha2 ||
            slice_target[2] !== 8'ha3 || slice_target[3] !== 8'ha4) begin
            $display("FAIL fixed_array_assignment concatenation");
            $finish;
        end

        slice_target = {slice_target[2:3], slice_target[0:1]};
        if (slice_target[0] !== 8'ha3 || slice_target[1] !== 8'ha4 ||
            slice_target[2] !== 8'ha1 || slice_target[3] !== 8'ha2) begin
            $display("FAIL fixed_array_assignment overlap_copy");
            $finish;
        end

        slice_target = '{default: 8'h77, 1: 8'hb1, 3: 8'hb3};
        if (slice_target[0] !== 8'h77 || slice_target[1] !== 8'hb1 ||
            slice_target[2] !== 8'h77 || slice_target[3] !== 8'hb3) begin
            $display("FAIL fixed_array_assignment keyed_pattern");
            $finish;
        end
        slice_target[2:3] = '{default: 8'h00, 2: 8'hc2, 3: 8'hc3};
        if (slice_target[2] !== 8'hc2 || slice_target[3] !== 8'hc3) begin
            $display("FAIL fixed_array_assignment keyed_slice_pattern");
            $finish;
        end

        state_source = '{4'b1xz0, 4'bz010};
        two_state_target = bit_array_t'(state_source);
        if (two_state_target[0] !== 4'b1000 || two_state_target[1] !== 4'b0010) begin
            $display("FAIL fixed_array_assignment two_state_conversion");
            $finish;
        end

        real_source = '{1.5, -2.25};
        real_target = real_source;
        if (real_target[0] != 1.5 || real_target[1] != -2.25) begin
            $display("FAIL fixed_array_assignment real_conversion");
            $finish;
        end

        nba_source = '{8'h51, 8'h52};
        nba_target = '{8'h00, 8'h00};
        nba_target <= nba_source;
        #1;
        if (nba_target[0] !== 8'h51 || nba_target[1] !== 8'h52) begin
            $display("FAIL fixed_array_assignment nonblocking");
            $finish;
        end

    $display("PASS fixed_array_assignment");
        $finish;
    end
endmodule
