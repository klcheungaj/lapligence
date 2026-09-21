// IEEE 1364-2001 §4.1.13 Table 28 / IEEE 1800-2009 §11.4.11 Table 11-20:
// packed ambiguous-selector Z/Z branch bits produce X.
module tb;
    reg [3:0] states;
    reg [15:0] known_zero;
    reg [15:0] known_one;
    reg [15:0] x_select;
    reg [15:0] z_select;
    reg [7:0] mixed_a;
    reg [7:0] mixed_b;
    reg [7:0] mixed_result;
    reg [64:0] wide_a;
    reg [64:0] wide_b;
    reg [64:0] wide_result;
    reg constant_result;
    reg runtime_selector;
    integer left;
    integer right;
    integer index;
    localparam frontend_constant = 1'bx ? 1'bz : 1'bz;
    initial begin
        states[0] = 1'b0;
        states[1] = 1'b1;
        states[2] = 1'bx;
        states[3] = 1'bz;
        known_zero = 16'b0;
        known_one = 16'b0;
        x_select = 16'b0;
        z_select = 16'b0;
        for (left = 0; left < 4; left = left + 1) begin
            for (right = 0; right < 4; right = right + 1) begin
                index = left * 4 + right;
                known_zero[index] = 1'b0 ? states[left] : states[right];
                known_one[index] = 1'b1 ? states[left] : states[right];
                x_select[index] = 1'bx ? states[left] : states[right];
                z_select[index] = 1'bz ? states[left] : states[right];
            end
        end

        mixed_a = 8'b10zz01x0;
        mixed_b = mixed_a;
        runtime_selector = 1'bx;
        mixed_result = runtime_selector ? mixed_a : mixed_b;
        wide_a = {65{1'bz}};
        wide_b = wide_a;
        wide_result = runtime_selector ? wide_a : wide_b;
        constant_result = 1'bx ? 1'bz : 1'bz;

        $display("known0=%b", known_zero);
        $display("known1=%b", known_one);
        $display("x=%b", x_select);
        $display("z=%b", z_select);
        $display("constant=%b frontend=%b mixed=%b wide=%b", constant_result,
                 frontend_constant, mixed_result, wide_result);
        $finish(0);
    end
endmodule
