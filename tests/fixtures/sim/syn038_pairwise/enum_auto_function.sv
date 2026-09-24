// IEEE 1800-2009 §§6.19, 6.21, 9.2.2.4, 10.4.2, 10.9 and 13.4.1–13.4.2.
module tb;
    typedef enum logic [7:0] {
        PHASE_IDLE = 8'h01,
        PHASE_ACTIVE = 8'h05,
        PHASE_DONE = 8'h07
    } phase_t;

    logic clk;
    logic [7:0] initial_code;
    logic [7:0] clock_code;
    phase_t initial_result;
    phase_t state;

    function automatic phase_t decode_phase(input logic [7:0] encoded);
        automatic phase_t local_phase = '{
            encoded[7], encoded[6], encoded[5], encoded[4],
            encoded[3], encoded[2], encoded[1], encoded[0]
        };
        return local_phase;
    endfunction

    always_ff @(posedge clk) begin
        state <= decode_phase(clock_code);
    end

    initial begin
        clk = 1'b0;
        initial_code = 8'h05;
        clock_code = 8'h07;
        initial_result = decode_phase(initial_code);

        #1 clk = 1'b1;
        #1;
        $display("initial=%b state=%b", initial_result, state);
        $finish(0);
    end
endmodule
