"""
Ọmọ Kọ́dà Economic Simulation Suite

Simulates the economy of the Agent OS including:
- Dopamine pool (global compute capacity)
- Synapse budget (per-agent cognitive budget with decay)
- Reputation scoring (dynamic difficulty formula)
- Tier progression (0-5 based on reputation)
"""

import json
from dataclasses import dataclass, field
from typing import List, Dict, Tuple
from enum import Enum
import random
import math


class Tier(Enum):
    """Agent reputation tier levels"""
    NOVICE = 0        # 0.000 - 10.000
    APPRENTICE = 1    # 10.000 - 25.000
    JOURNEYMAN = 2    # 25.000 - 50.000
    EXPERT = 3        # 50.000 - 75.000
    MASTER = 4        # 75.000 - 90.000
    SOVEREIGN = 5     # 90.000 - 100.000


# Economy constants
GLOBAL_DOPAMINE_POOL = 86_000_000_000  # 86 billion
MAX_SYNAPSE_PER_AGENT = 86_000_000      # 86 million
SYNAPSE_DECAY_RATE = 0.01               # 1% per day (matches OSOVM constants.jl)
DOPAMINE_DECAY_RATE = 0.01              # 1% per day (return to pool)
DAYS_PER_SIMULATION = 365


@dataclass
class Agent:
    """Represents an agent in the economic simulation"""
    id: str
    synapse: float = field(default=MAX_SYNAPSE_PER_AGENT)
    reputation: float = field(default=0.0)
    tier: Tier = field(default=Tier.NOVICE)
    tasks_completed: int = field(default=0)
    tasks_failed: int = field(default=0)
    reputation_updates: List[float] = field(default_factory=list)
    synapse_history: List[float] = field(default_factory=list)

    def get_tier(self) -> Tier:
        """Get tier based on current reputation"""
        if self.reputation < 10.0:
            return Tier.NOVICE
        elif self.reputation < 25.0:
            return Tier.APPRENTICE
        elif self.reputation < 50.0:
            return Tier.JOURNEYMAN
        elif self.reputation < 75.0:
            return Tier.EXPERT
        elif self.reputation < 90.0:
            return Tier.MASTER
        else:
            return Tier.SOVEREIGN

    def update_tier(self):
        """Update agent's tier based on reputation"""
        self.tier = self.get_tier()

    def decay_synapse(self, rate: float = SYNAPSE_DECAY_RATE):
        """Apply Synapse decay (1% per day)"""
        decay_amount = self.synapse * rate
        self.synapse = max(0, self.synapse - decay_amount)
        self.synapse_history.append(self.synapse)

    def gain_reputation(self, amount: float):
        """Increase agent reputation"""
        self.reputation = min(100.0, self.reputation + amount)
        self.reputation_updates.append(self.reputation)
        self.update_tier()

    def lose_reputation(self, amount: float):
        """Decrease agent reputation"""
        self.reputation = max(0.0, self.reputation - amount)
        self.reputation_updates.append(self.reputation)
        self.update_tier()


@dataclass
class EconomicState:
    """Represents the state of the economy at a point in time"""
    day: int
    dopamine_pool: float
    agents: Dict[str, Agent]
    total_synapse_allocated: float = 0.0
    total_synapse_decayed: float = 0.0
    total_reputation_earned: float = 0.0

    def calculate_total_synapse(self) -> float:
        """Calculate total Synapse allocated across all agents"""
        return sum(agent.synapse for agent in self.agents.values())

    def calculate_average_reputation(self) -> float:
        """Calculate average reputation across all agents"""
        if not self.agents:
            return 0.0
        return sum(agent.reputation for agent in self.agents.values()) / len(self.agents)

    def get_agents_by_tier(self) -> Dict[Tier, int]:
        """Count agents by tier"""
        tier_counts = {tier: 0 for tier in Tier}
        for agent in self.agents.values():
            tier_counts[agent.tier] += 1
        return tier_counts


class ReputationFormula:
    """Dynamic difficulty reputation formula"""

    @staticmethod
    def calculate_reputation_gain(
        task_difficulty: float,
        success_rate: float,
        current_reputation: float
    ) -> float:
        """
        Calculate reputation gain using dynamic difficulty formula.
        
        As reputation increases, it becomes harder to gain more (higher task difficulty).
        Formula: gain = (task_difficulty × success_rate) / (1 + current_reputation/10)
        """
        if success_rate <= 0:
            return 0.0

        # Normalize inputs
        difficulty_normalized = max(0.1, min(10.0, task_difficulty))
        success_normalized = max(0.0, min(1.0, success_rate))
        rep_normalized = max(0.0, current_reputation)

        # Calculate gain with scaling
        gain = (difficulty_normalized * success_normalized) / (1.0 + rep_normalized / 10.0)

        return min(10.0, gain)  # Cap at 10 points per task

    @staticmethod
    def calculate_reputation_loss(failure_severity: float, current_reputation: float) -> float:
        """
        Calculate reputation loss for failed tasks.
        
        Higher reputation agents lose more reputation for failures.
        Formula: loss = failure_severity × (1 + current_reputation/50)
        """
        severity_normalized = max(0.0, min(1.0, failure_severity))
        rep_normalized = max(0.0, current_reputation)

        loss = severity_normalized * (1.0 + rep_normalized / 50.0)
        return min(5.0, loss)  # Cap at 5 points loss per failure


class EconomicSimulation:
    """Simulates the economic dynamics of the Agent OS"""

    def __init__(self, num_agents: int = 100):
        self.num_agents = num_agents
        self.agents: Dict[str, Agent] = {}
        self.history: List[EconomicState] = []

        # Initialize agents
        for i in range(num_agents):
            agent_id = f"agent_{i:04d}"
            self.agents[agent_id] = Agent(id=agent_id)

        # Record initial state
        initial_state = EconomicState(
            day=0,
            dopamine_pool=GLOBAL_DOPAMINE_POOL,
            agents=self.agents
        )
        self.history.append(initial_state)

    def simulate_day(self, day: int):
        """Simulate one day of economic activity"""
        current_dopamine = self.history[-1].dopamine_pool

        # 1. Agents decay Synapse
        for agent in self.agents.values():
            agent.decay_synapse()

        # 2. Agents perform tasks
        for agent in self.agents.values():
            self._simulate_agent_activity(agent)

        # 3. Dopamine decays and returns to pool
        dopamine_decay = current_dopamine * DOPAMINE_DECAY_RATE
        new_dopamine = current_dopamine - dopamine_decay + dopamine_decay

        # 4. Record state
        state = EconomicState(
            day=day,
            dopamine_pool=new_dopamine,
            agents=self.agents,
            total_synapse_allocated=sum(a.synapse for a in self.agents.values()),
        )
        self.history.append(state)

    def _simulate_agent_activity(self, agent: Agent):
        """Simulate daily activity for an agent"""
        # Probability of task attempt based on Synapse availability
        if agent.synapse < 100_000:
            return  # Not enough Synapse to attempt tasks

        # Determine task difficulty based on tier
        difficulty_by_tier = {
            Tier.NOVICE: 1.0,
            Tier.APPRENTICE: 2.0,
            Tier.JOURNEYMAN: 3.0,
            Tier.EXPERT: 4.0,
            Tier.MASTER: 5.0,
            Tier.SOVEREIGN: 6.0,
        }

        task_difficulty = difficulty_by_tier[agent.tier]

        # Simulate task attempt
        success_rate = 0.7 - (agent.tier.value * 0.1)  # Harder tasks = lower success rate
        success_rate = max(0.1, success_rate)

        if random.random() < success_rate:
            agent.tasks_completed += 1
            reputation_gain = ReputationFormula.calculate_reputation_gain(
                task_difficulty, success_rate, agent.reputation
            )
            agent.gain_reputation(reputation_gain)
        else:
            agent.tasks_failed += 1
            reputation_loss = ReputationFormula.calculate_reputation_loss(0.5, agent.reputation)
            agent.lose_reputation(reputation_loss)

    def run(self, num_days: int = DAYS_PER_SIMULATION):
        """Run the economic simulation"""
        print(f"Starting economic simulation: {self.num_agents} agents, {num_days} days")
        for day in range(1, num_days + 1):
            self.simulate_day(day)
            if day % 30 == 0:
                print(f"  Day {day}: {self._get_progress_summary()}")

    def _get_progress_summary(self) -> str:
        """Get a one-line summary of current economic state"""
        state = self.history[-1]
        avg_rep = state.calculate_average_reputation()
        tier_distribution = state.get_agents_by_tier()
        sovereigns = tier_distribution[Tier.SOVEREIGN]
        return f"Avg Rep: {avg_rep:.2f}, Sovereigns: {sovereigns}"

    def get_report(self) -> Dict:
        """Generate a comprehensive report of the simulation"""
        final_state = self.history[-1]

        # Calculate statistics
        reputation_values = [agent.reputation for agent in self.agents.values()]
        synapse_values = [agent.synapse for agent in self.agents.values()]

        report = {
            "simulation_days": len(self.history) - 1,
            "num_agents": self.num_agents,
            "final_dopamine_pool": final_state.dopamine_pool,
            "reputation_stats": {
                "average": sum(reputation_values) / len(reputation_values),
                "min": min(reputation_values),
                "max": max(reputation_values),
                "median": sorted(reputation_values)[len(reputation_values) // 2],
            },
            "synapse_stats": {
                "total_allocated": sum(synapse_values),
                "average_per_agent": sum(synapse_values) / len(synapse_values),
                "min": min(synapse_values),
                "max": max(synapse_values),
                "agents_depleted": sum(1 for s in synapse_values if s < 1000),
            },
            "tier_distribution": {
                tier.name: count
                for tier, count in final_state.get_agents_by_tier().items()
            },
            "task_statistics": {
                "total_completed": sum(a.tasks_completed for a in self.agents.values()),
                "total_failed": sum(a.tasks_failed for a in self.agents.values()),
                "success_rate": sum(a.tasks_completed for a in self.agents.values()) /
                               (sum(a.tasks_completed + a.tasks_failed for a in self.agents.values()) or 1),
            },
        }

        return report

    def export_to_json(self, filename: str = "simulation_report.json"):
        """Export the full report to JSON"""
        report = self.get_report()
        with open(filename, 'w') as f:
            json.dump(report, f, indent=2)
        print(f"Report exported to {filename}")


if __name__ == "__main__":
    # Run a simulation
    sim = EconomicSimulation(num_agents=100)
    sim.run(num_days=365)

    # Print report
    report = sim.get_report()
    print("\n=== ECONOMIC SIMULATION REPORT ===")
    print(f"Simulation Duration: {report['simulation_days']} days")
    print(f"Number of Agents: {report['num_agents']}")
    print(f"Final Dopamine Pool: {report['final_dopamine_pool']:,.0f}")
    print(f"\nReputation Distribution:")
    print(f"  Average: {report['reputation_stats']['average']:.2f}")
    print(f"  Min: {report['reputation_stats']['min']:.2f}")
    print(f"  Max: {report['reputation_stats']['max']:.2f}")
    print(f"\nTier Distribution:")
    for tier, count in report['tier_distribution'].items():
        print(f"  {tier}: {count}")
    print(f"\nTask Statistics:")
    print(f"  Total Completed: {report['task_statistics']['total_completed']}")
    print(f"  Total Failed: {report['task_statistics']['total_failed']}")
    print(f"  Success Rate: {report['task_statistics']['success_rate']:.1%}")

    # Export to JSON
    sim.export_to_json("simulation_report.json")
