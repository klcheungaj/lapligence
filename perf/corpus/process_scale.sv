`ifndef LLG_SCALE_N
`define LLG_SCALE_N 100000
`endif
module process_scale;
    import "DPI-C" function void llg_scale_snapshot();
    event park;
    integer i;
    initial begin
        for (i = 0; i < `LLG_SCALE_N; i++) begin
            fork
                begin
                    @park;
                end
            join_none
        end
        #1;
        llg_scale_snapshot();
        $display("process_scale n=%0d parked", `LLG_SCALE_N);
        #1;
        $finish(0);
    end
endmodule
