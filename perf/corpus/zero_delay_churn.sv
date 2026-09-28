`ifndef LLG_CORPUS_N
`define LLG_CORPUS_N 10000
`endif

`ifndef LLG_CORPUS_ROUNDS
`define LLG_CORPUS_ROUNDS 200
`endif

module zero_delay_churn #(
    parameter integer N = `LLG_CORPUS_N,
    parameter integer ROUNDS = `LLG_CORPUS_ROUNDS
);
    event ping;
    event pong;
    logic [N-1:0] observed = '0;
    integer completed = 0;

    genvar i;
    for (i = 0; i < N; i = i + 1) begin : observers
        if ((i & 1) == 0) begin : ping_side
            always begin
                @ping;
                #0 observed[i] = ~observed[i];
            end
        end else begin : pong_side
            always begin
                @pong;
                #0 observed[i] = ~observed[i];
            end
        end
    end

    initial begin : responder
        forever begin
            @ping;
            #0 -> pong;
        end
    end

    initial begin : driver
        repeat (ROUNDS) begin
            #0 -> ping;
            @pong;
            completed = completed + 1;
            #0;
        end
        #1;
        $display("zero_delay_churn n=%0d rounds=%0d completed=%0d ones=%0d",
                 N, ROUNDS, completed, $countones(observed));
        $finish(0);
    end
endmodule
