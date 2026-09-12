// IEEE 1800-2009 7.5 and 7.6: dynamic arrays own compatible element values,
// including real and string values, across new[], copy, resize, and delete.
module tb;
    real samples[];
    real sample_copy[];
    string words[];
    string word_copy[];

    initial begin
        samples = new[2];
        samples[0] = 1.5;
        samples[1] = 2.5;
        sample_copy = samples;
        samples[0] = 9.5;
        if (sample_copy.size() !== 2 ||
            sample_copy[0] != 1.5 || sample_copy[1] != 2.5) begin
            $display("FAIL dynamic_value_arrays real_copy");
            $finish;
        end
        samples = new[3](samples);
        if (samples.size() !== 3 || samples[0] != 9.5 ||
            samples[1] != 2.5 || samples[2] != 0.0) begin
            $display("FAIL dynamic_value_arrays real_resize");
            $finish;
        end

        words = new[2];
        words[0] = "alpha";
        words[1] = "beta";
        word_copy = words;
        words[0] = "changed";
        if (word_copy.size() !== 2 ||
            word_copy[0].len() !== 5 || word_copy[1].len() !== 4) begin
            $display("FAIL dynamic_value_arrays string_copy");
            $finish;
        end
        words = new[3](words);
        if (words.size() !== 3 || words[0].len() !== 7 ||
            words[1].len() !== 4 || words[2].len() !== 0) begin
            $display("FAIL dynamic_value_arrays string_resize");
            $finish;
        end

        samples.delete();
        words.delete();
        if (samples.size() !== 0 || words.size() !== 0) begin
            $display("FAIL dynamic_value_arrays delete");
            $finish;
        end
        $display("PASS dynamic_value_arrays");
        $finish;
    end
endmodule
