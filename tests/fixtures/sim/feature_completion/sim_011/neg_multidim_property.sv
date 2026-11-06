// SIM-011 boundary: a multidimensional fixed-array class property is legal
// (SV 7.4.2, 8.4); it is used whole, by element or through memory file
// tasks, and selecting a row is rejected with a dedicated diagnostic.
class grid_c;
    int g[2][3];
endclass

module tb;
    grid_c h;

    initial begin
        h = new;
        $display("%p", h.g[1]);
    end
endmodule
