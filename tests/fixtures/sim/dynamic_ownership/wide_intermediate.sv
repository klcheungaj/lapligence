module tb;
    logic [6:0] narrow;
    initial begin
        narrow = 7'h55;
        $display("%0d %0d", $bits({1024{narrow}}), $countones({1024{narrow}}));
        $finish(0);
    end
endmodule
