module tb;
    function automatic void take(ref static logic [7:0] source);
        source = 8'h01;
    endfunction

    function automatic void caller();
        logic [7:0] local_value;
        take(local_value);
    endfunction

    initial begin
        caller();
        $finish(0);
    end
endmodule
