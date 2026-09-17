module tb;
    initial begin
        for (int index = 0; index < 3; index++) begin
            automatic int saved = index;
            fork
                begin
                    #1;
                    $display("%0d", saved);
                end
            join_none
        end
        wait fork;
        $finish(0);
    end
endmodule
