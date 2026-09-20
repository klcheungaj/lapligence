// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_calls.sv
// IEEE 1800-2009 12.7.3 and 13: formal and automatic fixed-array values.
module tb;
    typedef logic [1:0][3:2] packed_t;
    typedef packed_t vector_t [1:0];
    vector_t source;
    vector_t result;
    integer local_visits;

    function automatic integer count(input vector_t data);
        count = 0;
        foreach (data[i,j,k]) count += data[i][j][k];
    endfunction

    function automatic vector_t flipped(input vector_t data);
        vector_t local_data;
        foreach (local_data[i,j,k]) local_data[i][j][k] = ~data[i][j][k];
        return local_data;
    endfunction

    initial begin
        source[1] = 4'b1010;
        source[0] = 4'b0100;
        result = flipped(source);
        local_visits = 0;
        begin : local_scope
            automatic vector_t local_data;
            local_data = result;
            foreach (local_data[i,j,k]) local_visits++;
        end
        $display("source=%b,%b result=%b,%b counts=%0d,%0d local=%0d",
                 source[1], source[0], result[1], result[0],
                 count(source), count(result), local_visits);
        $finish(0);
    end
endmodule
