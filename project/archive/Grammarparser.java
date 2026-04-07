import java.io.*;
import java.util.*;

public class GrammarParser {

    // Production rule: LHS -> RHS
    static class Rule {
        int id;
        String lhs;
        List<String> rhs;

        Rule(int id, String lhs, List<String> rhs) {
            this.id = id;
            this.lhs = lhs;
            this.rhs = new ArrayList<>(rhs);
        }
    }

    // Determines whether a token is a nonterminal
    static boolean isNonTerminal(String s) {
        if (s.equals("lambda") || s.equals("|") || s.equals("->")) {
            return false;
        }
        for (char c : s.toCharArray()) {
            if (Character.isUpperCase(c)) {
                return true;
            }
        }
        return false;
    }

    // Checks if a symbol directly derives lambda
    static boolean derivesToLambda(String symbol, List<Rule> grammar) {
        for (Rule rule : grammar) {
            if (rule.lhs.equals(symbol)) {
                for (String token : rule.rhs) {
                    if (token.equals("lambda")) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    // Computes the FIRST set for a sequence of symbols
    static Set<String> firstSet(List<String> sequence, Set<String> visited, List<Rule> grammar) {
        Set<String> F = new LinkedHashSet<>();

        if (sequence.isEmpty()) {
            return F;
        }

        String X = sequence.get(0);

        if (!isNonTerminal(X)) {
            F.add(X);
            return F;
        }

        if (!visited.contains(X)) {
            visited.add(X);

            for (Rule p : grammar) {
                if (p.lhs.equals(X)) {
                    Set<String> G = firstSet(p.rhs, visited, grammar);
                    F.addAll(G);
                }
            }
        }

        if (derivesToLambda(X, grammar) && sequence.size() > 1) {
            List<String> beta = sequence.subList(1, sequence.size());
            Set<String> G = firstSet(beta, visited, grammar);
            F.addAll(G);
        }

        return F;
    }

    // Computes FOLLOW sets for all nonterminals using the standard iterative algorithm
    static Map<String, Set<String>> computeFollowSets(
            Set<String> nonTerminals, String startSymbol,
            List<Rule> grammar) {

        // Initialize each nonterminal's FOLLOW set to empty
        Map<String, Set<String>> follow = new LinkedHashMap<>();
        for (String nt : nonTerminals) {
            follow.put(nt, new TreeSet<>());
        }

        // Rule 1: place $ in FOLLOW(start symbol)
        if (!startSymbol.isEmpty()) {
            follow.get(startSymbol).add("$");
        }

        // Iterate until no FOLLOW set changes
        boolean changed = true;
        while (changed) {
            changed = false;

            for (Rule rule : grammar) {
                List<String> rhs = rule.rhs;

                for (int i = 0; i < rhs.size(); i++) {
                    String B = rhs.get(i);
                    if (!isNonTerminal(B) || B.equals("lambda")) continue;

                    // beta = everything after B in this production
                    List<String> beta = rhs.subList(i + 1, rhs.size());

                    // Rule 2: add FIRST(beta) \ {lambda} to FOLLOW(B)
                    Set<String> visited = new LinkedHashSet<>();
                    Set<String> firstBeta = firstSet(beta, visited, grammar);
                    firstBeta.remove("lambda");

                    Set<String> followB = follow.get(B);
                    if (followB.addAll(firstBeta)) changed = true;

                    // Rule 3: if beta is empty or entirely derives lambda,
                    //         add FOLLOW(LHS) to FOLLOW(B)
                    boolean betaDerivesLambda = beta.isEmpty() || beta.stream()
                            .allMatch(sym -> {
                                Set<String> v = new LinkedHashSet<>();
                                Set<String> f = firstSet(Collections.singletonList(sym), v, grammar);
                                return f.contains("lambda") || sym.equals("lambda");
                            });

                    if (betaDerivesLambda) {
                        Set<String> followLHS = follow.get(rule.lhs);
                        if (followLHS != null && followB.addAll(followLHS)) changed = true;
                    }
                }
            }
        }

        return follow;
    }

    public static void main(String[] args) throws IOException {
        if (args.length < 1) {
            System.err.println("Usage: java GrammarParser <input_file>");
            return;
        }

        // Read all tokens from the file into a flat list
        List<String> tokens = new ArrayList<>();
        try (Scanner scanner = new Scanner(new File(args[0]))) {
            while (scanner.hasNext()) {
                tokens.add(scanner.next());
            }
        }

        List<Rule> grammar = new ArrayList<>();
        Set<String> nonTerminals = new TreeSet<>();
        Set<String> allSymbols = new TreeSet<>();
        String startSymbol = "";
        String currentLHS = "";
        int ruleCount = 1;

        // Parse rules from the token list
        for (int i = 0; i < tokens.size(); i++) {
            if ((i + 1 < tokens.size()) && tokens.get(i + 1).equals("->")) {
                currentLHS = tokens.get(i);
                i++; // Skip the arrow
            } else {
                // Build a new rule for currentLHS
                List<String> rhs = new ArrayList<>();

                nonTerminals.add(currentLHS);
                allSymbols.add(currentLHS);

                while (i < tokens.size()) {
                    String tok = tokens.get(i);

                    if (tok.equals("|")) {
                        break; // End of this production; same LHS continues
                    }
                    if ((i + 1 < tokens.size()) && tokens.get(i + 1).equals("->")) {
                        i--; // Backtrack so the outer loop re-reads the next LHS
                        break;
                    }

                    rhs.add(tok);

                    if (!tok.equals("lambda")) {
                        allSymbols.add(tok);
                        if (tok.equals("$")) {
                            startSymbol = currentLHS;
                        }
                    }
                    i++;
                }

                grammar.add(new Rule(ruleCount++, currentLHS, rhs));
            }
        }

        // --- Output ---

        System.out.println("Grammar Non-Terminals:");
        System.out.println(String.join(", ", nonTerminals));

        System.out.println("\nGrammar Symbols:");
        System.out.println(String.join(", ", allSymbols));

        System.out.println("\nGrammar Rules:");
        for (Rule r : grammar) {
            System.out.print("(" + r.id + ") " + r.lhs + " -> ");
            System.out.println(String.join(" ", r.rhs));
        }

        System.out.println("\nGrammar Start Symbol or Goal: " + startSymbol);

        System.out.println("\nFirst Sets for Nonterminals:");
        for (String nt : nonTerminals) {
            Set<String> visited = new LinkedHashSet<>();
            Set<String> result = firstSet(Collections.singletonList(nt), visited, grammar);

            System.out.print("FIRST(" + nt + ") = { ");
            System.out.print(String.join(", ", result));
            System.out.println(" }");
        }

        Map<String, Set<String>> followSets = computeFollowSets(nonTerminals, startSymbol, grammar);

        System.out.println("\nFollow Sets for Nonterminals:");
        for (String nt : nonTerminals) {
            System.out.print("FOLLOW(" + nt + ") = { ");
            System.out.print(String.join(", ", followSets.get(nt)));
            System.out.println(" }");
        }
    }
}